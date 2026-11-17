// v0.3-M9 C5 (plan #17): the persisted scope-exit release plan.
//
// WHY (High tier: ownership / data integrity): codegen (C6/C7) frees heap locals from this plan and
// derives nothing itself. A wrong fate is an early or a double free, or a leak. Each test asserts
// the plan's recorded fate at an exit edge, not a program's output: there is no release pass yet,
// so the miss is only observable here. One test per contract; a case is a separate test only when
// it takes a different checker path (a different edge kind, a different source of the fact).
//
//   fact                                   source of the fact
//   owner / not owner                      the C5a transfer record's anchor
//   given on every / some / no path        the live scope at the edge (`consumed_on_every_path`)
//   returned                               the returned value's holders
//   string move                            `TypedModule::string_moves` (C3b)
//   crossing                               a release is a read at its edge, in the one producer
//
// The corpus sweep is the exit test's parity check: for every fixture and example that type-checks,
// the plan agrees with `Scope::consumed_classes` (captured per edge) and with the transfer record's
// escapes (built by a separate path, `TransferRecorder::escape`).

#[macro_use]
mod support;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use support::{errors, run};
use ynz_ast::nodes::{Block, FunctionDecl, Item, Stmt};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::generics::GenericShapeTable;
use ynz_typeck::release_plan::{
    heap_class_of, Fate, HeapClass, Names, OmitReason, ReleaseKind, ReleasePlan, TypeTables,
};
use ynz_typeck::shapes::ShapeTable;
use ynz_typeck::types::Type;
use ynz_typeck::{check_query, CheckOutput};

fn function_of<'a>(output: &'a CheckOutput, name: &str) -> &'a FunctionDecl {
    output
        .typed_module
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no function `{name}`"))
}

/// `(user name, fate)` of the list at the end of `function`'s body.
fn body_end(output: &CheckOutput, function: &str) -> Vec<(String, Fate)> {
    let body = &function_of(output, function).body;
    let list = output
        .typed_module
        .release_plan
        .block_end(&body.span)
        .unwrap_or_else(|| panic!("no body-end list for `{function}`"));
    list.iter()
        .map(|entry| (entry.decl.2.clone(), entry.fate))
        .collect()
}

fn return_spans(block: &Block, found: &mut Vec<ynz_diagnostics::SourceSpan>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return { span, .. } => found.push(span.clone()),
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                return_spans(body, found)
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    return_spans(&arm.body, found);
                }
                if let Some(block) = else_arm {
                    return_spans(block, found);
                }
            }
            _ => {}
        }
    }
}

/// `(user name, fate)` per `return` of `function`, in source order.
fn return_edges(output: &CheckOutput, function: &str) -> Vec<Vec<(String, Fate)>> {
    let mut spans = Vec::new();
    return_spans(&function_of(output, function).body, &mut spans);
    spans
        .iter()
        .map(|span| {
            output
                .typed_module
                .release_plan
                .return_edge(span)
                .unwrap_or_else(|| panic!("no return list at {span:?}"))
                .iter()
                .map(|entry| (entry.decl.2.clone(), entry.fate))
                .collect()
        })
        .collect()
}

fn clean(source: &str) -> CheckOutput {
    let output = run(source);
    let found = errors(&output);
    assert!(found.is_empty(), "expected a clean program, got {found:#?}");
    assert_eq!(
        output.typed_module.release_plan.internal_errors(),
        Vec::<String>::new()
    );
    output
}

fn fate_of(list: &[(String, Fate)], name: &str) -> Fate {
    list.iter()
        .find(|(entry, _)| entry == name)
        .unwrap_or_else(|| panic!("`{name}` is not in {list:?}"))
        .1
}

const ARRAY: ReleaseKind = ReleaseKind::Array;

// ── Fates at the end of a body ──────────────────────────────────────────────────────────────

/// Bug: a plain owner is left out of its scope's list, so it leaks.
#[test]
fn a_plain_local_is_released_at_the_end_of_its_block() {
    let output = clean(
        "function plain() -> nothing {\n  let rows = [1, 2, 3]\n  print(`${rows.count()}`)\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "plain"), "rows"),
        Fate::Release(ARRAY)
    );
}

/// Bug: a binding given away on every path is released too (double free with the callee).
#[test]
fn a_binding_given_on_every_path_is_omitted_as_moved() {
    let output = clean(&format!(
        "{}function giver() -> nothing {{\n  let rows = [1, 2, 3]\n  eat(rows)\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&output, "giver"), "rows"),
        Fate::Omitted(OmitReason::Moved)
    );
}

/// Bug: a binding given on SOME paths is released unconditionally (double free on the give path)
/// or never (a leak on the other); it needs the run-time test, and a move site that nulls the slot.
#[test]
fn a_binding_given_on_some_paths_is_released_if_live_and_has_a_move_site() {
    let output = clean(&format!(
        "{}function some(flag: boolean) -> nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&output, "some"), "rows"),
        Fate::ReleaseIfLive(ARRAY)
    );
    let sites: Vec<_> = output.typed_module.release_plan.move_sites().collect();
    assert_eq!(sites.len(), 1, "one read moves `rows`: {sites:?}");
    assert_eq!(sites[0].1.founder.2, "rows");
}

/// Bug: a `return` before the give omits the binding (a leak on that path), or the list at the
/// body end releases it after the give: each edge has its OWN fate, from the path to it.
#[test]
fn an_early_return_before_a_give_still_releases_the_binding() {
    let output = clean(&format!(
        "{}function early(flag: boolean) -> nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    return\n  }}\n  eat(rows)\n}}\n",
        support::EAT
    ));
    let returns = return_edges(&output, "early");
    assert_eq!(fate_of(&returns[0], "rows"), Fate::Release(ARRAY));
    assert_eq!(
        fate_of(&body_end(&output, "early"), "rows"),
        Fate::Omitted(OmitReason::Moved)
    );
}

/// Bug: the returned value is released at the `return` (the caller gets a freed array).
#[test]
fn the_returned_binding_is_omitted_at_its_return() {
    let output = clean(
        "function make() -> array<int> {\n  let rows = [1, 2, 3]\n  let spare = [4, 5]\n  return rows\n}\n",
    );
    let returns = return_edges(&output, "make");
    assert_eq!(
        fate_of(&returns[0], "rows"),
        Fate::Omitted(OmitReason::Returned)
    );
    assert_eq!(fate_of(&returns[0], "spare"), Fate::Release(ARRAY));
}

/// Bug: a parameter the caller owns is released by the callee; a `give` parameter is not, because
/// the callee owns it.
#[test]
fn a_borrowed_parameter_is_the_callers_and_a_give_parameter_is_released() {
    let output = clean(
        "function borrowed(share rows: array<int>) -> nothing {\n  print(`${rows.count()}`)\n}\nfunction owned(give rows: array<int>) -> nothing {\n  print(`${rows.count()}`)\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "borrowed"), "rows"),
        Fate::Omitted(OmitReason::CallerOwns)
    );
    assert_eq!(
        fate_of(&body_end(&output, "owned"), "rows"),
        Fate::Release(ARRAY)
    );
}

/// Bug: a reassigned owner leaks the value each assignment replaces (`Omitted(Reassigned)`), or its
/// assignment frees a value it no longer owns: the plan releases it at its exit and records the
/// release its assignment makes.
#[test]
fn a_reassigned_owner_is_released_and_its_assignment_releases_the_old_value() {
    let output = clean(
        "function again() -> nothing {\n  let rows = [1, 2, 3]\n  rows = [4, 5]\n  print(`${rows.count()}`)\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "again"), "rows"),
        Fate::Release(ARRAY)
    );
    let function = function_of(&output, "again");
    let stmt_span = match &function.body.stmts[1] {
        Stmt::Assign { span, .. } => span.clone(),
        other => panic!("expected the assignment, got {other:?}"),
    };
    let release = output
        .typed_module
        .release_plan
        .assign_release(&stmt_span)
        .expect("the assignment releases the old value");
    assert_eq!(release.decl.2, "rows");
    assert_eq!(release.kind, ARRAY);
}

/// Bug: a generic body is left out of the plan ("every scope"), or its type-parameter values are
/// dropped from it (a given `T` leaked in every instance).
#[test]
fn a_generic_body_is_covered_and_its_parameter_values_are_planned() {
    let output = clean(
        "function keep<T>(give value: T) -> T {\n  let spare = [1, 2]\n  print(`${spare.count()}`)\n  return value\n}\n",
    );
    let list = body_end(&output, "keep");
    assert_eq!(fate_of(&list, "spare"), Fate::Release(ARRAY));
    // The value is returned: the caller owns it from then on, whatever `T` is.
    assert_eq!(
        fate_of(&return_edges(&output, "keep")[0], "value"),
        Fate::Omitted(OmitReason::Returned)
    );
}

/// Bug (High: leak): a given `T` that nothing hands on was omitted as a type parameter, so every
/// instance leaked it. It is a releasing fate of `ReleaseKind::TypeParameter`, which codegen
/// resolves by the instance's type; stored on some paths only, it is released only if live.
#[test]
fn a_given_type_parameter_value_has_a_releasing_fate() {
    let output = clean(
        "function drop<T>(give value: T) -> nothing {\n  print(`x`)\n}\nfunction maybeKeep<T>(give value: T, flag: boolean) -> array<T> {\n  let items: array<T> = []\n  if (flag) {\n    items.add(value)\n  }\n  return items\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "drop"), "value"),
        Fate::Release(ReleaseKind::TypeParameter)
    );
    assert_eq!(
        fate_of(&body_end(&output, "maybeKeep"), "value"),
        Fate::ReleaseIfLive(ReleaseKind::TypeParameter)
    );
}

/// Bug: a list is recorded for the function body but not for a nested block (a loop body's
/// per-iteration locals leak once per iteration).
#[test]
fn a_loop_body_has_its_own_list() {
    let output = clean(
        "function looped() -> nothing {\n  let count = 3\n  while (count > 0) {\n    let row = [1]\n    print(`${row.count()}`)\n    count = count - 1\n  }\n}\n",
    );
    let function = function_of(&output, "looped");
    let Stmt::While { body, .. } = &function.body.stmts[1] else {
        panic!("the loop is the second statement")
    };
    let list = output
        .typed_module
        .release_plan
        .block_end(&body.span)
        .expect("the loop body has a list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].fate, Fate::Release(ARRAY));
}

/// Bug: the propagation edge is missing, so an errors function that returns early on a failed
/// call leaks every owner it holds.
#[test]
fn an_auto_propagation_lists_every_open_owner() {
    let output = clean(
        "function risky() -> int errors {\n  return 1\n}\nfunction chained() -> int errors {\n  let rows = [1, 2]\n  let value = risky()\n  return value\n}\n",
    );
    let function = function_of(&output, "chained");
    let Stmt::Return {
        value: Some(ynz_ast::nodes::Expr::Ident(_, span)),
        ..
    } = &function.body.stmts[2]
    else {
        panic!("the third statement returns the errors-capable value")
    };
    let list = output
        .typed_module
        .release_plan
        .propagation_edge(span)
        .expect("the read of `value` fires an auto-propagation");
    assert!(list
        .iter()
        .any(|entry| entry.decl.2 == "rows" && entry.fate == Fate::Release(ARRAY)));
}

// ── Strings: one fact, with `string_moves` ──────────────────────────────────────────────────

/// Bug: a string with no read after it is treated as moved (leaks) or a last-use read that a store
/// may take is released unconditionally (double free with the store's owner).
#[test]
fn a_string_is_released_if_live_exactly_when_the_record_lists_a_read() {
    let output = clean(
        "function words() -> nothing {\n  let unused = `plain`\n  let label = `n=${3}`\n  print(label)\n}\n",
    );
    let list = body_end(&output, "words");
    assert_eq!(fate_of(&list, "unused"), Fate::Release(ReleaseKind::String));
    assert_eq!(
        fate_of(&list, "label"),
        Fate::ReleaseIfLive(ReleaseKind::String)
    );
    // The plan's move sites ARE the record's last-use reads of released strings: one fact.
    let plan = &output.typed_module.release_plan;
    let record = &output.typed_module.string_moves;
    for (span, site) in plan.move_sites() {
        assert!(
            record.contains(span.0, span.1),
            "{span:?} is not in string_moves"
        );
        assert_eq!(
            record.last_use_binding(span.0, span.1),
            Some(&site.founder),
            "the record and the plan name different bindings for {span:?}"
        );
    }
    assert!(plan.move_sites().count() >= 1);
}

// ── What a type needs ───────────────────────────────────────────────────────────────────────

/// Bug: a type that owns heap is classified as owning none (a leak for good: C6 has nothing to
/// release it from) or the reverse (a value type is listed). High tier: this table decides which
/// bindings are ever freed, so every heap-owning kind has a case.
#[test]
fn heap_class_of_each_type() {
    let shapes = ShapeTable::default();
    let generic_shapes = GenericShapeTable::default();
    let tables = TypeTables {
        shapes: &shapes,
        generic_shapes: &generic_shapes,
    };
    let array = Type::BuiltinArray {
        elem: Box::new(Type::Int),
    };
    let string_list = Type::BuiltinFixed {
        elem: Box::new(Type::String),
        size: ynz_typeck::FixedLength::Unknown,
    };
    let cases: Vec<(Type, HeapClass)> = vec![
        (Type::Int, HeapClass::NoHeap),
        (Type::Bool, HeapClass::NoHeap),
        (Type::String, HeapClass::Heap(ReleaseKind::String)),
        (array.clone(), HeapClass::Heap(ARRAY)),
        (
            Type::BuiltinMap {
                key: Box::new(Type::Int),
                val: Box::new(Type::Int),
            },
            HeapClass::Heap(ReleaseKind::Map),
        ),
        (
            Type::Maybe {
                inner: Box::new(Type::Int),
            },
            HeapClass::NoHeap,
        ),
        (
            Type::Maybe {
                inner: Box::new(array.clone()),
            },
            HeapClass::Heap(ReleaseKind::Maybe),
        ),
        // A union with no shape variant and one variant beside `nothing` is that variant's raw
        // pointer (`is_envelope_union`): the slot holds what its one heap variant holds. With two
        // variants beside `nothing`, one of them heap (`string | array<int>`, `int | array<int>`,
        // `string | channel<int>`), the slot is tagged and releases by the variant its tag names.
        (
            Type::Union {
                variants: vec![Type::Nothing, array.clone()],
            },
            HeapClass::Heap(ARRAY),
        ),
        (
            Type::Union {
                variants: vec![Type::Int, array.clone()],
            },
            HeapClass::Heap(ReleaseKind::Union),
        ),
        (
            Type::Union {
                variants: vec![Type::String, array.clone()],
            },
            HeapClass::Heap(ReleaseKind::Union),
        ),
        (
            Type::Union {
                variants: vec![
                    Type::String,
                    Type::BuiltinChannel {
                        elem: Box::new(Type::Int),
                    },
                ],
            },
            HeapClass::Heap(ReleaseKind::Union),
        ),
        (
            Type::Union {
                variants: vec![Type::Int, Type::Bool],
            },
            HeapClass::NoHeap,
        ),
        (string_list, HeapClass::Heap(ReleaseKind::Fixed)),
        (
            Type::BuiltinFixed {
                elem: Box::new(Type::Int),
                size: ynz_typeck::FixedLength::Unknown,
            },
            HeapClass::NoHeap,
        ),
        (
            Type::Dynamic {
                contract: "Drawable".into(),
            },
            HeapClass::Heap(ReleaseKind::Dynamic),
        ),
        (
            Type::ErrorsCapable {
                inner: Box::new(Type::Int),
            },
            HeapClass::Heap(ReleaseKind::ErrorsCapable),
        ),
        (
            Type::Maybe {
                inner: Box::new(Type::TypeParam { name: "T".into() }),
            },
            HeapClass::TypeParameter,
        ),
        (
            Type::TypeParam { name: "T".into() },
            HeapClass::TypeParameter,
        ),
        // A shape the tables do not describe is assumed to own heap: leaking is the wrong error.
        (
            Type::Shape {
                name: "Unknown".into(),
            },
            HeapClass::Heap(ReleaseKind::NestedShape),
        ),
    ];
    for (ty, expected) in cases {
        assert_eq!(heap_class_of(&ty, &tables), expected, "{ty:?}");
    }
}

/// Bug: a shape's heap is judged by its name or its first field: a shape of ints owns nothing
/// (not listed), a shape with a string owns heap, a shape holding a shape owns that shape's cell.
#[test]
fn shapes_are_classified_by_their_fields() {
    let output = clean(
        "shape Label {\n  text: string\n}\nshape Plain {\n  size: int\n}\nshape Holder {\n  inner: Plain\n}\nfunction risky() -> int errors {\n  return 1\n}\nfunction kinds() -> nothing {\n  let label: Label = { text: `a` }\n  let plain: Plain = { size: 1 }\n  let holder: Holder = { inner: { size: 2 } }\n  let words: fixed<string> = [`a`, `b`]\n  let result = risky()\n  if (result.failed()) {\n    print(`bad`)\n  }\n  print(plain.size.toString())\n  print(label.text)\n  print(holder.inner.size.toString())\n  print(words.count().toString())\n  print(result.toString())\n}\n",
    );
    let list = body_end(&output, "kinds");
    assert_eq!(fate_of(&list, "label"), Fate::Release(ReleaseKind::Shape));
    assert_eq!(
        fate_of(&list, "holder"),
        Fate::Release(ReleaseKind::NestedShape)
    );
    assert_eq!(fate_of(&list, "words"), Fate::Release(ReleaseKind::Fixed));
    assert_eq!(
        fate_of(&list, "result"),
        Fate::Release(ReleaseKind::ErrorsCapable)
    );
    assert!(
        list.iter().all(|(name, _)| name != "plain"),
        "a shape of ints owns no heap, so it is not listed: {list:?}"
    );
}

/// Bug: reading a kind with no frame slot across a suspension as a release rejects programs that
/// compile today (Check 2/2b); only a kind with frame backing feeds the crossing set.
#[test]
fn only_a_kind_with_frame_backing_feeds_the_crossing_set() {
    let output = clean(
        "shape Label {\n  text: string\n}\nfunction sleeper() -> nothing {\n  let words: fixed<string> = [`a`, `b`]\n  let label: Label = { text: `a` }\n  let rows = [1, 2]\n  wait sleep(1)\n}\n",
    );
    assert_eq!(
        crossing_names(&output, "sleeper", Names::User),
        vec!["label".to_string(), "rows".to_string(), "words".to_string()],
        "the array, the flat shape and the `fixed` list cross (each has a frame home)"
    );
    assert!(ReleaseKind::Array.has_frame_backing());
    assert!(ReleaseKind::Shape.has_frame_backing());
    assert!(ReleaseKind::Fixed.has_frame_backing());
    assert!(!ReleaseKind::Dynamic.has_frame_backing());
}

// ── The crossing set ────────────────────────────────────────────────────────────────────────

fn crossing_names(output: &CheckOutput, function: &str, names: Names) -> Vec<String> {
    let function = function_of(output, function);
    let params: Vec<&str> = function
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    let reads = output
        .typed_module
        .release_plan
        .reads(&function.name, names);
    ynz_typeck::crossing_local_names(
        &function.body.stmts,
        &params,
        &HashSet::new(),
        &output.typed_module.expr_types,
        &reads,
    )
}

/// Bug: an owner declared before a `wait` and released after it has no frame slot, so the release
/// frees a stack address. Not crossing here would be the S3 probe `e1`.
#[test]
fn an_owner_declared_before_a_wait_crosses_it() {
    let output =
        clean("function sleeper() -> nothing {\n  let rows = [1, 2, 3]\n  wait sleep(1)\n}\n");
    assert_eq!(
        crossing_names(&output, "sleeper", Names::User),
        vec!["rows".to_string()]
    );
}

/// Bug: the crossing set is widened to every owner (an extra frame slot and flush per owner, above
/// hand-written cost): an owner no suspension separates from its release must not cross.
#[test]
fn an_owner_declared_after_the_last_wait_does_not_cross() {
    let output = clean(
        "function late() -> nothing {\n  wait sleep(1)\n  let rows = [1, 2, 3]\n  print(`${rows.count()}`)\n}\n",
    );
    assert_eq!(
        crossing_names(&output, "late", Names::User),
        Vec::<String>::new()
    );
}

/// Bug: a `return` after a wait releases the outer owner without a frame slot. The other arm gives
/// the owner away and the returning arm never reaches the body end, so the `return` edge is the
/// ONLY release that follows the wait: the body-end read alone would not make the owner cross.
#[test]
fn an_owner_crosses_a_wait_to_reach_a_return_release() {
    let output = clean(&format!(
        "{}function guarded(choice: int) -> nothing {{\n  let rows = [1, 2, 3]\n  if (choice) {{\n    1 => {{ eat(rows) }}\n    else => {{\n      wait sleep(1)\n      return\n    }}\n  }}\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&output, "guarded"), "rows"),
        Fate::Omitted(OmitReason::Moved),
        "the body end does not release it"
    );
    assert_eq!(
        fate_of(&return_edges(&output, "guarded")[0], "rows"),
        Fate::Release(ARRAY)
    );
    assert_eq!(
        crossing_names(&output, "guarded", Names::User),
        vec!["rows".to_string()]
    );
}

// ── Parity over the corpus ──────────────────────────────────────────────────────────────────

fn ynz_files(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            ynz_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "ynz") {
            found.push(path);
        }
    }
}

/// How much of each fate the corpus exercised: a parity check over empty lists proves nothing.
#[derive(Default)]
struct Tally {
    edges: AtomicUsize,
    released: AtomicUsize,
    released_if_live: AtomicUsize,
    moved: AtomicUsize,
    returned: AtomicUsize,
    views: AtomicUsize,
    heap_moves: AtomicUsize,
    kinds: Mutex<HashSet<ReleaseKind>>,
}

/// Findings for one file: empty when it does not type-check (not this test's subject) or the plan
/// agrees with the live scope and the transfer record at every edge.
fn parity_findings(path: &Path, tally: &Tally) -> (bool, Vec<String>) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return (false, Vec::new());
    };
    let db = CompilerDb::default();
    let file = SourceFile::new(&db, path.display().to_string(), contents);
    let output = (*check_query(&db, file)).clone();
    if !errors(&output).is_empty() {
        return (false, Vec::new());
    }
    let plan: &ReleasePlan = &output.typed_module.release_plan;
    let mut findings: Vec<String> = plan
        .internal_errors()
        .iter()
        .map(|message| format!("{}: {message}", path.display()))
        .collect();
    findings.extend(
        plan.parity_violations(&output.typed_module.transfers)
            .into_iter()
            .map(|message| format!("{}: {message}", path.display())),
    );
    tally.views.fetch_add(
        output.typed_module.non_owned_reads.view_count(),
        Ordering::Relaxed,
    );
    tally
        .heap_moves
        .fetch_add(output.typed_module.heap_moves.len(), Ordering::Relaxed);
    findings.extend(
        c1f_agreement(&output)
            .into_iter()
            .map(|message| format!("{}: {message}", path.display())),
    );
    // Every list is total over its block: a fate for every binding the edge lists, none twice.
    for edge in plan.facts() {
        tally.edges.fetch_add(1, Ordering::Relaxed);
        for entry in &edge.entries {
            if let HeapClass::Heap(kind) = entry.heap {
                tally.kinds.lock().expect("mutex").insert(kind);
            }
            match entry.fate {
                Fate::Release(_) => tally.released.fetch_add(1, Ordering::Relaxed),
                Fate::ReleaseIfLive(_) => tally.released_if_live.fetch_add(1, Ordering::Relaxed),
                Fate::Omitted(OmitReason::Moved) => tally.moved.fetch_add(1, Ordering::Relaxed),
                Fate::Omitted(OmitReason::Returned) => {
                    tally.returned.fetch_add(1, Ordering::Relaxed)
                }
                Fate::Omitted(_) => 0,
            };
        }
        let mut seen = HashSet::new();
        for entry in &edge.entries {
            if !seen.insert(entry.decl.clone()) {
                findings.push(format!(
                    "{}: `{}` is listed twice at {:?} {:?}",
                    path.display(),
                    entry.decl.2,
                    edge.kind,
                    edge.span
                ));
            }
        }
    }
    (true, findings)
}

/// Exit test 2: over every fixture and example that type-checks, the plan agrees with
/// `Scope::consumed_classes` and the transfer record for every scope (every edge, every function,
/// generic bodies included). Independent files are checked in parallel.
#[test]
fn the_plan_agrees_with_the_live_scope_and_the_record_over_the_whole_corpus() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut corpus = Vec::new();
    ynz_files(&root.join("crates/ynz-driver/tests/fixtures"), &mut corpus);
    ynz_files(&root.join("examples"), &mut corpus);
    corpus.sort();
    assert!(
        corpus.len() > 200,
        "the corpus is missing: {} files",
        corpus.len()
    );
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .min(4);
    let cursor = AtomicUsize::new(0);
    let findings: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let checked = AtomicUsize::new(0);
    let tally = Tally::default();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(path) = corpus.get(index) else { break };
                let (type_checked, mut found) = parity_findings(path, &tally);
                if type_checked {
                    checked.fetch_add(1, Ordering::Relaxed);
                }
                findings.lock().expect("mutex").append(&mut found);
            });
        }
    });
    let findings = findings.into_inner().expect("mutex");
    assert!(
        checked.load(Ordering::Relaxed) > 150,
        "too few files type-checked to mean anything: {}",
        checked.load(Ordering::Relaxed)
    );
    for (name, count) in [
        ("released", &tally.released),
        ("released if live", &tally.released_if_live),
        ("moved", &tally.moved),
        ("returned", &tally.returned),
        ("view", &tally.views),
        ("heap move", &tally.heap_moves),
    ] {
        assert!(
            count.load(Ordering::Relaxed) > 0,
            "the corpus exercised no `{name}` fate over {} edges",
            tally.edges.load(Ordering::Relaxed)
        );
    }
    // Every kind the corpus holds is classified; none is omitted for being unsupported (the
    // parity check rejects an omission without a reason). Print what it exercised on failure.
    let kinds = tally.kinds.lock().expect("mutex").clone();
    for expected in [
        ReleaseKind::String,
        ReleaseKind::Array,
        ReleaseKind::Map,
        ReleaseKind::Shape,
        ReleaseKind::Maybe,
        ReleaseKind::Union,
    ] {
        assert!(
            kinds.contains(&expected),
            "the corpus never exercised {expected:?}; it did {kinds:?}"
        );
    }
    assert!(
        findings.is_empty(),
        "{} findings:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

/// Bug: a move that nulls the founder's slot is not seen as a write to it, so the state machine
/// never flushes the nulled slot and a resumed release frees the moved value (S3 probe `c2`).
#[test]
fn a_statement_holding_a_move_site_names_its_founder_as_written() {
    let output = clean(&format!(
        "{}function some(flag: boolean) -> nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n  print(`done`)\n}}\n",
        support::EAT
    ));
    let function = function_of(&output, "some");
    let plan = &output.typed_module.release_plan;
    assert_eq!(
        plan.founders_moved_in(&function.body.stmts[1]),
        vec!["rows".to_string()],
        "the `if` holding the give writes `rows`' slot"
    );
    assert!(
        plan.founders_moved_in(&function.body.stmts[2]).is_empty(),
        "a statement with no move site writes no slot"
    );
}

// ── Fixer round 1 (code): the propagated error leaves the function; a shadowed binding stays ─

/// Bug: the auto-propagation returns the failed value's error to the caller, yet the edge lists
/// the failed binding as `Release` (the error object is freed and returned: a double free). The
/// edge must omit it as returned, and the transfer record must say it escapes by return.
/// Producer: `Checker::resolve_ident` recorded the propagation edge with no returned classes.
#[test]
fn an_auto_propagation_omits_the_propagated_binding_as_returned() {
    let output = clean(
        "function risky() -> int errors {\n  return 1\n}\nfunction chained() -> int errors {\n  let rows = [1, 2]\n  let value = risky()\n  return value\n}\n",
    );
    let function = function_of(&output, "chained");
    let Stmt::Return {
        value: Some(ynz_ast::nodes::Expr::Ident(_, span)),
        ..
    } = &function.body.stmts[2]
    else {
        panic!("the third statement returns the errors-capable value")
    };
    let list: Vec<(String, Fate)> = output
        .typed_module
        .release_plan
        .propagation_edge(span)
        .expect("the read of `value` fires an auto-propagation")
        .iter()
        .map(|entry| (entry.decl.2.clone(), entry.fate))
        .collect();
    assert_eq!(
        fate_of(&list, "value"),
        Fate::Omitted(OmitReason::Returned),
        "the propagated error is the function's result"
    );
    assert_eq!(fate_of(&list, "rows"), Fate::Release(ARRAY));
    assert!(
        output
            .typed_module
            .release_plan
            .parity_violations(&output.typed_module.transfers)
            .is_empty(),
        "plan and record agree"
    );
}

/// Bug: `let rows` twice in one block replaced the first binding in the scope's frame, so the first
/// array was in no list at all (a leak on every path). Both bindings leave scope with the block.
/// Producer: `Scope::insert` dropped the shadowed entry.
#[test]
fn a_binding_shadowed_in_its_own_block_is_still_released() {
    let output = clean(
        "function twice() -> nothing {\n  let rows = [1]\n  let rows = [2]\n  print(`${rows.count()}`)\n}\n",
    );
    let list = body_end(&output, "twice");
    assert_eq!(
        list.iter().filter(|(name, _)| name == "rows").count(),
        2,
        "both `rows` bindings are listed: {list:?}"
    );
    assert!(
        list.iter().all(|(_, fate)| *fate == Fate::Release(ARRAY)),
        "{list:?}"
    );
}

/// Bug: a shadowed owner is judged by the wrong binding of its name (the scope forgets it or freezes
/// its state at the shadowing). `return passed` copies `passed` at the `return` (a string, array, map
/// or shape the return does not move is copied), so the caller holds a value of its own and both
/// `rows` are released; before the copy-owner change the return marked the whole class returned.
#[test]
fn a_shadowed_owner_follows_its_class_when_a_class_mate_is_returned() {
    let output = clean(
        "function pass<T>(share rows: T) -> T {\n  return rows\n}\nfunction shadowed() -> array<int> {\n  let rows = [1]\n  let passed = pass(rows)\n  let rows = [9]\n  return passed\n}\n",
    );
    let returns = return_edges(&output, "shadowed");
    let list = &returns[0];
    let firsts: Vec<Fate> = list
        .iter()
        .filter(|(name, _)| name == "rows")
        .map(|(_, fate)| *fate)
        .collect();
    assert_eq!(firsts.len(), 2, "both `rows` are listed: {list:?}");
    assert_eq!(
        firsts.iter().filter(|fate| fate.releases()).count(),
        2,
        "both `rows` release, the caller holds a copy of `passed`: {list:?}"
    );
    assert!(
        output
            .typed_module
            .release_plan
            .parity_violations(&output.typed_module.transfers)
            .is_empty(),
        "{list:?}"
    );
}

/// Bug: one `background` spawn that gives two bindings is one read; the plan kept the first
/// founder only, so the second (released if live) had no move site and the module failed with an
/// internal error (High: data integrity). Producer: `note_move_site` keyed sites by span alone.
#[test]
fn a_spawn_that_gives_two_bindings_moves_both_founders() {
    let output = clean(
        "function work(first: array<int>, second: array<int>) -> nothing {\n  print(`${first.count()}`)\n}\nfunction spawns(flag: boolean) -> nothing {\n  let first: array<int> = [1]\n  let second: array<int> = [2]\n  if (flag) {\n    background work(first, second)\n  }\n}\n",
    );
    let list = body_end(&output, "spawns");
    assert_eq!(fate_of(&list, "first"), Fate::ReleaseIfLive(ARRAY));
    assert_eq!(fate_of(&list, "second"), Fate::ReleaseIfLive(ARRAY));
    let function = function_of(&output, "spawns");
    let mut founders = output
        .typed_module
        .release_plan
        .founders_moved_in(&function.body.stmts[2]);
    founders.sort();
    assert_eq!(founders, vec!["first".to_string(), "second".to_string()]);
}

// ── Fixer round 1 (test): the rules each fate comes from, at their boundaries ────────────────

/// Bug: the join of two branches decides "given on every path" wrongly (`any` for `all`, or a
/// returning arm counted as a path): a give on one arm only is omitted (a leak on the other arm),
/// or a give on both arms is released if live (a needless run-time test), or an arm that returns
/// makes the join look "some paths" when the only path that reaches it gave.
#[test]
fn a_join_is_moved_only_when_every_path_that_reaches_it_gave() {
    let both = clean(&format!(
        "{}function both(choice: int) -> nothing {{\n  let rows = [1, 2, 3]\n  if (choice) {{\n    1 => {{ eat(rows) }}\n    else => {{ eat(rows) }}\n  }}\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&both, "both"), "rows"),
        Fate::Omitted(OmitReason::Moved)
    );
    let one = clean(&format!(
        "{}function one(choice: int) -> nothing {{\n  let rows = [1, 2, 3]\n  if (choice) {{\n    1 => {{ eat(rows) }}\n    else => {{ print(`kept`) }}\n  }}\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&one, "one"), "rows"),
        Fate::ReleaseIfLive(ARRAY)
    );
    let other_returns = clean(&format!(
        "{}function other(choice: int) -> nothing {{\n  let rows = [1, 2, 3]\n  if (choice) {{\n    1 => {{ eat(rows) }}\n    else => {{ return }}\n  }}\n}}\n",
        support::EAT
    ));
    assert_eq!(
        fate_of(&body_end(&other_returns, "other"), "rows"),
        Fate::Omitted(OmitReason::Moved),
        "the returning arm never reaches the join"
    );
    assert_eq!(
        fate_of(&return_edges(&other_returns, "other")[0], "rows"),
        Fate::Release(ARRAY),
        "the returning arm still owns the array at its `return`"
    );
}

/// Bug: a string's `ReleaseIfLive` is decided by whether the record lists any read of it, not by
/// whether one can precede THIS edge: the early `return` then frees through a run-time test the
/// path never needs, and (reversed) a read before the edge is missed and the store's owner and the
/// scope both free the string.
#[test]
fn a_string_is_released_if_live_only_at_edges_a_listed_read_can_precede() {
    let output = clean(
        "function late(flag: boolean) -> nothing {\n  let label = `n=${3}`\n  if (flag) {\n    return\n  }\n  print(label)\n}\n",
    );
    assert_eq!(
        fate_of(&return_edges(&output, "late")[0], "label"),
        Fate::Release(ReleaseKind::String),
        "no listed read comes before this `return`"
    );
    assert_eq!(
        fate_of(&body_end(&output, "late"), "label"),
        Fate::ReleaseIfLive(ReleaseKind::String)
    );
}

/// Bug: a string parameter is released by the callee (the caller still owns it: a double free).
#[test]
fn a_string_parameter_is_not_released_by_the_callee() {
    let output = clean("function show(text: string) -> nothing {\n  print(text)\n}\n");
    assert_eq!(
        fate_of(&body_end(&output, "show"), "text"),
        Fate::Omitted(OmitReason::NotOwner)
    );
}

/// Bug: a string given to a `give` parameter is released by both sides (a double free) or by
/// neither (a leak). The checker refuses a read of it after the call, like any given value, so the
/// caller omits it as moved and the callee, which owns what it was given, releases it: exactly
/// one side. A `share` string parameter stays the caller's (`NotOwner` in the callee). Decided
/// from `IMP-strings.md` (a string is moved or copied by a STORE; a call hands nothing over by
/// itself) and the checker's own give: the sink consumes the caller's binding.
#[test]
fn a_give_string_parameter_is_released_by_the_callee_and_omitted_by_the_caller() {
    let output = clean(
        "function consume(give text: string) -> nothing {\n  print(text)\n}\nfunction borrow(share text: string) -> nothing {\n  print(text)\n}\nfunction caller() -> nothing {\n  let given = `n=${3}`\n  let lent = `m=${4}`\n  consume(given)\n  borrow(lent)\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "consume"), "text"),
        Fate::Release(ReleaseKind::String),
        "the callee owns what it was given"
    );
    assert_eq!(
        fate_of(&body_end(&output, "borrow"), "text"),
        Fate::Omitted(OmitReason::NotOwner),
        "a `share` string stays the caller's"
    );
    let caller = body_end(&output, "caller");
    assert_eq!(
        fate_of(&caller, "given"),
        Fate::Omitted(OmitReason::Moved),
        "the caller gave it away on every path"
    );
    assert!(
        fate_of(&caller, "lent").releases(),
        "the caller still owns a string it only lent"
    );
}

/// A call result is an owner of its own value, whatever the callee's declared type: a generic
/// callee's `return` copies a heap-typed result at its instance, so `second` holds an array of its
/// own and releases it (as `first` releases its own); the class `first` founds does not reach it.
/// Bug (leak): the result was judged a piece of `first` (`MayAlias`, the type parameter unresolved)
/// and neither bound by move nor released.
#[test]
fn a_generic_call_result_is_an_owner_of_its_own_value() {
    let output = clean(
        "function pass<T>(share rows: T) -> T {\n  return rows\n}\nfunction mate() -> nothing {\n  let first = [1]\n  let second = pass(first)\n  print(`${second.count()}`)\n}\n",
    );
    let list = body_end(&output, "mate");
    assert_eq!(fate_of(&list, "first"), Fate::Release(ARRAY));
    assert_eq!(fate_of(&list, "second"), Fate::Release(ARRAY));
}

/// Bug: a binding that outlives the owner of the field it reads (`cell = owner.rows`, `owner` dies
/// at the end of the `if`) is left pointing at what the scope exit frees (a use after free), or
/// both are left unreleased (a leak). The read is a copy that outlives its root, so `cell` owns it
/// and both are released at their own scope exits (decided: Chief Architect, replacing the run-time
/// drop-flag ruling for `OutlivedByNonOwner`; `transfer_record.rs` pins the no-static-founder case
/// that is left).
#[test]
fn a_read_that_outlives_its_root_is_a_copy_released_at_both_scope_ends() {
    let output = clean(
        "shape Bucket {\n  rows: array<int>\n}\nfunction make() -> Bucket {\n  let bucket: Bucket = { rows: [9] }\n  return bucket\n}\nfunction under(condition: boolean) -> int {\n  let cell: array<int> = make().rows\n  if (condition) {\n    let owner: Bucket = make()\n    cell = owner.rows\n  }\n  return cell.count()\n}\n",
    );
    let function = function_of(&output, "under");
    let Stmt::If { body, .. } = &function.body.stmts[1] else {
        panic!("the `if` is the second statement")
    };
    let inner: Vec<(String, Fate)> = output
        .typed_module
        .release_plan
        .block_end(&body.span)
        .expect("the `if` body has a list")
        .iter()
        .map(|entry| (entry.decl.2.clone(), entry.fate))
        .collect();
    assert_eq!(fate_of(&inner, "owner"), Fate::Release(ReleaseKind::Shape));
    assert!(fate_of(&body_end(&output, "under"), "cell").releases());
}

/// Bug: a `return` lists the frames outermost first, or each frame first-declared first: codegen
/// releases in list order, so a later local would be freed after what it borrows from.
#[test]
fn a_return_lists_the_innermost_frame_first_and_the_last_declared_first() {
    let output = clean(
        "function ordered(flag: boolean) -> nothing {\n  let outer_first = [1]\n  let outer_second = [2]\n  if (flag) {\n    let inner_first = [3]\n    let inner_second = [4]\n    return\n  }\n  print(`${outer_first.count()}`)\n  print(`${outer_second.count()}`)\n}\n",
    );
    let names: Vec<String> = return_edges(&output, "ordered")[0]
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    assert_eq!(
        names,
        vec!["inner_second", "inner_first", "outer_second", "outer_first"]
    );
}

/// Bug: a kind of heap value (or a type holding one) is classified as owning none, so C6 has
/// nothing to release it from; or a value type is listed. The kinds `heap_class_of_each_type`
/// does not build by hand.
#[test]
fn heap_class_of_the_remaining_kinds() {
    let shapes = ShapeTable::default();
    let generic_shapes = GenericShapeTable::default();
    let tables = TypeTables {
        shapes: &shapes,
        generic_shapes: &generic_shapes,
    };
    let cases: Vec<(Type, HeapClass)> = vec![
        (
            Type::BuiltinChannel {
                elem: Box::new(Type::Int),
            },
            HeapClass::Heap(ReleaseKind::Channel),
        ),
        (
            Type::BackgroundHandle {
                result: Box::new(Type::Int),
                msg_elem: None,
            },
            HeapClass::Heap(ReleaseKind::TaskHandle),
        ),
        (
            Type::Sensitive {
                inner: Box::new(Type::String),
            },
            HeapClass::Heap(ReleaseKind::String),
        ),
        (
            Type::Sensitive {
                inner: Box::new(Type::Int),
            },
            HeapClass::NoHeap,
        ),
        (
            Type::MapEntry {
                key: Box::new(Type::Int),
                val: Box::new(Type::String),
            },
            HeapClass::Heap(ReleaseKind::MapEntry),
        ),
        (
            Type::MapEntry {
                key: Box::new(Type::Int),
                val: Box::new(Type::Int),
            },
            HeapClass::NoHeap,
        ),
        (
            Type::MapEntry {
                key: Box::new(Type::Int),
                val: Box::new(Type::TypeParam { name: "V".into() }),
            },
            HeapClass::TypeParameter,
        ),
        (
            Type::Union {
                variants: vec![Type::Int, Type::TypeParam { name: "T".into() }],
            },
            HeapClass::TypeParameter,
        ),
        // One heap-owning variant decides, even beside a type parameter.
        (
            Type::Union {
                variants: vec![
                    Type::TypeParam { name: "T".into() },
                    Type::BuiltinArray {
                        elem: Box::new(Type::Int),
                    },
                ],
            },
            HeapClass::Heap(ReleaseKind::Union),
        ),
        (
            Type::BuiltinFixed {
                elem: Box::new(Type::TypeParam { name: "T".into() }),
                size: ynz_typeck::FixedLength::Unknown,
            },
            HeapClass::TypeParameter,
        ),
    ];
    for (ty, expected) in cases {
        assert_eq!(heap_class_of(&ty, &tables), expected, "{ty:?}");
    }
}

/// Bug: a generic shape instance is judged by its declaration, not its arguments (`Pair<int, int>`
/// listed, or `Pair<string, int>` not), and a shape that holds itself recurses forever or is
/// classed as owning nothing.
#[test]
fn generic_and_recursive_shapes_are_classified_by_what_they_hold() {
    let output = clean(
        "shape Pair<A, B> {\n  first: A\n  second: B\n}\nshape Node {\n  label: string\n  next: maybe<Node>\n}\nfunction pairs(give numbers: Pair<int, int>, give named: Pair<string, int>, give chain: Node) -> nothing {\n  print(numbers.first.toString())\n  print(named.first)\n  print(chain.label)\n}\n",
    );
    let list = body_end(&output, "pairs");
    assert!(
        list.iter().all(|(name, _)| name != "numbers"),
        "a pair of ints owns no heap: {list:?}"
    );
    assert_eq!(fate_of(&list, "named"), Fate::Release(ReleaseKind::Shape));
    assert_eq!(
        fate_of(&list, "chain"),
        Fate::Release(ReleaseKind::Shape),
        "a recursive shape owns heap (its label, its boxed link)"
    );
}

/// Bug: an owner omitted from every edge is still read there, so it gets a frame slot and a flush
/// for nothing (above hand-written cost); a binding released only if live is missed and its slot
/// is not preserved across the wait.
#[test]
fn only_the_owners_that_release_cross_a_wait() {
    let moved = clean(&format!(
        "{}function moved() -> nothing {{\n  let rows = [1, 2, 3]\n  eat(rows)\n  wait sleep(1)\n}}\n",
        support::EAT
    ));
    assert_eq!(
        crossing_names(&moved, "moved", Names::User),
        Vec::<String>::new(),
        "given on every path: no release reads it after the wait"
    );
    let if_live = clean(&format!(
        "{}function if_live(flag: boolean) -> nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n  wait sleep(1)\n}}\n",
        support::EAT
    ));
    assert_eq!(
        crossing_names(&if_live, "if_live", Names::User),
        vec!["rows".to_string()],
        "released if live after the wait"
    );
    let borrowed =
        clean("function borrowed(share rows: array<int>) -> nothing {\n  wait sleep(1)\n}\n");
    assert_eq!(
        crossing_names(&borrowed, "borrowed", Names::User),
        Vec::<String>::new(),
        "the caller owns it: no release"
    );
}

/// Bug: the reads a release adds are placed at the wrong statement (after the wrong `return`, in
/// the wrong block, or twice), so the one crossing producer sees a use before a suspension it
/// follows, or after one it precedes. `with_reads` is the only place the placement is decided.
#[test]
fn with_reads_adds_one_read_per_owner_at_each_edge_it_belongs_to() {
    use ynz_ast::nodes::Expr;
    let output = clean(&format!(
        "{}function placed(flag: boolean) -> nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    let inner = [4]\n    return\n  }}\n  print(`${{rows.count()}}`)\n}}\n",
        support::EAT
    ));
    let function = function_of(&output, "placed");
    let reads = output
        .typed_module
        .release_plan
        .reads("placed", Names::User);
    let rewritten = reads.with_reads(&function.body.stmts);
    let read_names = |stmts: &[Stmt]| -> Vec<String> {
        stmts
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Expr(Expr::Ident(name, _)) => Some(name.clone()),
                _ => None,
            })
            .collect()
    };
    // Top level: `let`, `if`, `print` (an expression statement, not an identifier), then the body
    // end's read of `rows`.
    assert_eq!(rewritten.len(), function.body.stmts.len() + 1);
    assert_eq!(read_names(&rewritten), vec!["rows".to_string()]);
    assert!(
        matches!(rewritten.last(), Some(Stmt::Expr(Expr::Ident(name, _))) if name == "rows"),
        "the body-end read comes last"
    );
    // The `if` body: `let`, `return`, then the `return` edge's reads, innermost frame first.
    let Stmt::If { body, .. } = &rewritten[1] else {
        panic!("the second statement is still the `if`")
    };
    assert_eq!(body.stmts.len(), 2 + 2 + 1);
    assert!(matches!(body.stmts[1], Stmt::Return { .. }));
    assert_eq!(
        read_names(&body.stmts[2..]),
        vec!["inner".to_string(), "rows".to_string(), "inner".to_string()],
        "the `return` edge lists the inner and outer owner; the arm's own end lists the inner"
    );
    // A function with no release reads is returned unchanged.
    let quiet = clean("function quiet() -> nothing {\n  print(`x`)\n}\n");
    let plain = function_of(&quiet, "quiet");
    assert_eq!(
        quiet
            .typed_module
            .release_plan
            .reads("quiet", Names::User)
            .with_reads(&plain.body.stmts)
            .len(),
        plain.body.stmts.len()
    );
}

/// Bug: the read a propagation edge adds is placed at the outer statement as well as inside the
/// nested block that holds the identifier (twice), or in the outer block only (after the nested
/// block's own end, where its locals are already gone). The edge belongs right after the
/// statement that holds the identifier, in the innermost block.
#[test]
fn with_reads_puts_a_propagation_read_after_its_statement_in_its_own_block() {
    use ynz_ast::nodes::Expr;
    let output = clean(
        "function risky() -> int errors {\n  return 1\n}\nfunction chained(flag: boolean) -> int errors {\n  let rows = [1, 2]\n  if (flag) {\n    let inner = [3]\n    let value = risky()\n    print(value.toString())\n  }\n  return 0\n}\n",
    );
    let function = function_of(&output, "chained");
    let rewritten = output
        .typed_module
        .release_plan
        .reads("chained", Names::User)
        .with_reads(&function.body.stmts);
    let read_names = |stmts: &[Stmt]| -> Vec<String> {
        stmts
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Expr(Expr::Ident(name, _)) => Some(name.clone()),
                _ => None,
            })
            .collect()
    };
    let Stmt::If { body, .. } = &rewritten[1] else {
        panic!("the second statement is the `if`")
    };
    assert!(
        matches!(body.stmts[2], Stmt::Expr(_)),
        "the statement holding the failing read is the `print` call"
    );
    assert_eq!(
        read_names(&body.stmts),
        vec![
            "inner".to_string(),
            "rows".to_string(),
            "value".to_string(),
            "inner".to_string()
        ],
        "the propagation edge's owners right after the `print` (not `value`, whose error it returns), then the block's own end, `value` (an errors-capable value has a frame home since C8 slice 4b) first"
    );
    assert_eq!(
        read_names(&rewritten),
        vec!["rows".to_string(), "rows".to_string()],
        "the outer block has only its `return` edge and its own end: no copy of the nested propagation edge, no read right after the `if`"
    );
}

// ── C1f's records: heap moves and views ─────────────────────────────────────────────────────

/// Every `let` of `block` (nested blocks included): `(value expression span, declaration)`.
fn lets_of(block: &Block, found: &mut Vec<((usize, usize), ynz_typeck::binding_names::DeclKey)>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let {
                name,
                name_span,
                value,
                ..
            } => {
                let span = value.span();
                found.push((
                    (span.start, span.end),
                    ynz_typeck::binding_names::decl_key(name_span, name),
                ));
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                lets_of(body, found)
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    lets_of(&arm.body, found);
                }
                if let Some(block) = else_arm {
                    lets_of(block, found);
                }
            }
            _ => {}
        }
    }
}

/// Where the release plan disagrees with C1f's records (`non_owned_reads`, `heap_moves`):
///
/// * a view owns nothing: at every edge it is listed, its fate omits it, and the founder of its
///   class is another binding;
/// * a store the checker records as a heap move consumed a binding: when the store is a plain
///   statement of the block that declares the founder, the founder's fate at that block's end is
///   `Omitted(Moved)`, and at every edge it is not `Release` for a store that precedes the edge
///   in the same straight line (checked at the declaring block's end).
fn c1f_agreement(output: &CheckOutput) -> Vec<String> {
    let module = &output.typed_module;
    let plan = &module.release_plan;
    let mut findings = Vec::new();

    let mut lets = Vec::new();
    for item in &module.module.items {
        if let Item::Function(function) = item {
            lets_of(&function.body, &mut lets);
        }
    }
    for (span, decl) in &lets {
        if !module.non_owned_reads.is_view(span.0, span.1) {
            continue;
        }
        for edge in plan.facts() {
            for entry in edge.entries.iter().filter(|entry| &entry.decl == decl) {
                if !matches!(
                    entry.fate,
                    Fate::Omitted(
                        OmitReason::NotOwner | OmitReason::CallerOwns | OmitReason::NoStaticFounder
                    )
                ) {
                    findings.push(format!(
                        "view `{}` has fate {:?} at {:?} {:?}",
                        entry.codegen_name, entry.fate, edge.kind, edge.span
                    ));
                }
            }
        }
        let anchor = module
            .binding_names
            .codegen_name(decl)
            .and_then(|name| {
                module
                    .transfers
                    .function(
                        &plan
                            .facts()
                            .iter()
                            .find(|edge| edge.entries.iter().any(|entry| &entry.decl == decl))?
                            .function,
                    )?
                    .class_of(name)
            })
            .map(|class| class.anchor.clone());
        if let Some(ynz_typeck::transfer_record::ReleaseAnchor::Founder { decl: founder, .. }) =
            anchor
        {
            if &founder == decl {
                findings.push(format!("view `{}` founds its own class", decl.2));
            }
        }
    }

    for ((start, end, _), decl) in module.binding_names.uses() {
        if !module.heap_moves.contains(*start, *end) {
            continue;
        }
        // The edges of the function that lists the moved binding, and the block that ends it.
        let Some(listing) = plan
            .facts()
            .iter()
            .find(|edge| edge.entries.iter().any(|entry| &entry.decl == decl))
        else {
            continue;
        };
        let function_edges: Vec<_> = plan
            .facts()
            .iter()
            .filter(|edge| edge.function == listing.function)
            .collect();
        let Some(own_end) = function_edges.iter().find(|edge| {
            edge.kind == ynz_typeck::release_plan::EdgeKind::BlockEnd
                && edge.entries.iter().any(|entry| &entry.decl == decl)
        }) else {
            continue;
        };
        let inside = |edge: &&ynz_typeck::release_plan::EdgeFacts| {
            edge.span.0 <= *start && *end <= edge.span.1
        };
        let nested_block = function_edges.iter().any(|edge| {
            edge.kind == ynz_typeck::release_plan::EdgeKind::BlockEnd
                && edge.span != own_end.span
                && inside(edge)
                && own_end.span.0 <= edge.span.0
                && edge.span.1 <= own_end.span.1
        });
        let in_return = function_edges
            .iter()
            .any(|edge| edge.kind == ynz_typeck::release_plan::EdgeKind::Return && inside(edge));
        if !inside(own_end) || nested_block || in_return {
            continue;
        }
        let entry = own_end
            .entries
            .iter()
            .find(|entry| &entry.decl == decl)
            .expect("listed at its own block end");
        // The moved binding is the founder or an alias of it: an alias is `NotOwner` and the
        // founder carries the move; a founder must be omitted, or released only if live while the
        // move nulls its slot (a reassigned owner given on every path to the edge: an assignment
        // may have revived it).
        let nulled_at_the_store = plan
            .move_sites()
            .any(|(span, site)| span == (*start, *end) && &site.founder == decl);
        let unsafe_release = match entry.fate {
            Fate::Release(_) => true,
            Fate::ReleaseIfLive(_) => !nulled_at_the_store,
            Fate::Omitted(_) => false,
        };
        if unsafe_release {
            findings.push(format!(
                "`{}` is moved by a store at {start}..{end} but released at the end of its block ({:?})",
                entry.codegen_name, entry.fate
            ));
        }
    }
    findings
}

/// Bug: a read-only view is listed as an owner (it frees what its root still owns: a double free)
/// or the view, not its root, founds the class.
#[test]
fn a_view_is_not_an_owner_and_its_root_stays_the_owner() {
    let output = clean(
        "shape Inner {\n  label: string\n}\nshape Outer {\n  inner: Inner\n  tag: int\n}\nfunction peek() -> nothing {\n  let first: Inner = { label: `a` }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(inner.label)\n}\n",
    );
    assert!(
        output.typed_module.non_owned_reads.view_count() > 0,
        "the program is meant to hold a view"
    );
    let list = body_end(&output, "peek");
    assert_eq!(fate_of(&list, "inner"), Fate::Omitted(OmitReason::NotOwner));
    assert!(fate_of(&list, "outer").releases());
    assert_eq!(c1f_agreement(&output), Vec::<String>::new());
}

/// Bug: a binding a store moves (`heap_moves`) is also released at its scope exit (a double free
/// of its boxes): the store owns it, so the moved binding is omitted and the new binding releases.
#[test]
fn a_binding_moved_by_a_store_is_omitted_and_the_new_owner_releases() {
    let output = clean(
        "shape Bucket {\n  rows: array<int>\n}\nfunction move() -> nothing {\n  let holder: Bucket = { rows: [1, 2] }\n  let moved = holder\n  let total = moved.rows.count()\n  print(`${total}`)\n}\n",
    );
    assert!(
        !output.typed_module.heap_moves.is_empty(),
        "the program is meant to hold a heap move"
    );
    let list = body_end(&output, "move");
    assert_eq!(fate_of(&list, "holder"), Fate::Omitted(OmitReason::Moved));
    assert!(fate_of(&list, "moved").releases());
    assert_eq!(c1f_agreement(&output), Vec::<String>::new());
}

// ── Fixer round 2 (code): the crossing set is name-based, so a name must mean one binding ──────

/// Bug (a regression of a program that compiled before C5): the release read of an array owner `reading`
/// in one block makes the NAME `reading` cross a `wait`, and the guards judge the name by the first
/// `let v` of the function, a `maybe<int>` that does not cross: the program is rejected with
/// "a `maybe<int>` value cannot yet cross a `wait`". Producer: `resolve_function` fed an owner's
/// user name to the name-based crossing producer when the function declares that name twice.
#[test]
fn a_name_declared_twice_in_a_function_feeds_no_release_read_to_the_guards() {
    let output = clean(
        "function twice(flag: boolean) -> nothing {\n  let numbers = [1]\n  if (flag) {\n    let reading: maybe<int> = numbers.get(0)\n    print(`${reading.or(0)}`)\n  }\n  if (flag) {\n    let reading = [1, 2]\n    wait sleep(1)\n  }\n}\n",
    );
    assert_eq!(
        crossing_names(&output, "twice", Names::User),
        vec!["numbers".to_string()],
        "`numbers` (one binding) crosses; the ambiguous `reading` is not fed, so no guard judges it by the wrong binding"
    );
    // Codegen's names are unique, so codegen still reads both owners at their edges.
    assert!(
        !output
            .typed_module
            .release_plan
            .reads("twice", Names::Codegen)
            .is_empty(),
        "codegen's reads keep the owners"
    );
}

// ── Fixer round 2 (test): the boundaries of the ownership rules ───────────────────────────────

/// Which edge of a function a table row reads.
#[derive(Clone, Copy, Debug)]
enum At {
    BodyEnd,
    /// The nth `return`, in source order.
    Return(usize),
}

fn list_at(output: &CheckOutput, function: &str, at: At) -> Vec<(String, Fate)> {
    match at {
        At::BodyEnd => body_end(output, function),
        At::Return(index) => return_edges(output, function)
            .into_iter()
            .nth(index)
            .unwrap_or_else(|| panic!("`{function}` has no return #{index}")),
    }
}

/// Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the
/// join (a nested `if`, a match arm, a branch that returns) turns an early or a double free into a
/// leak or the reverse. `rows` is given at most once; each row names the edge read and the fate
/// that edge must show. A `return` inside the construct that gave sees the give; an edge after a
/// join sees it only when every path that reaches the join gave.
#[test]
fn a_give_in_a_nested_if_or_a_match_arm_reaches_each_edge_by_the_paths_that_reach_it() {
    // (case, body after `let rows = [1]`, edge, expected fate of `rows`)
    let cases: [(&str, &str, At, Fate); 7] = [
        (
            "nested if, edge after the inner join, still inside the outer if",
            "  if (outer) {\n    if (inner) {\n      eat(rows)\n    }\n    return\n  }\n",
            At::Return(0),
            Fate::ReleaseIfLive(ARRAY),
        ),
        (
            "nested if whose outer branch always returns: the give never reaches the body end",
            "  if (outer) {\n    if (inner) {\n      eat(rows)\n    }\n    return\n  }\n",
            At::BodyEnd,
            Fate::Release(ARRAY),
        ),
        (
            "the same give in a match arm: some arms give, the others keep it",
            "  if (choice) {\n    1 => { eat(rows) }\n    2 => { print(`two`) }\n    else => { print(`other`) }\n  }\n",
            At::BodyEnd,
            Fate::ReleaseIfLive(ARRAY),
        ),
        (
            "a match arm that returns sees the array still owned at its `return`",
            "  if (choice) {\n    1 => { eat(rows) }\n    else => { return }\n  }\n",
            At::Return(0),
            Fate::Release(ARRAY),
        ),
        (
            "the arm that returns never reaches the join, so the join sees only the give",
            "  if (choice) {\n    1 => { eat(rows) }\n    else => { return }\n  }\n",
            At::BodyEnd,
            Fate::Omitted(OmitReason::Moved),
        ),
        (
            "every arm of an inner match gives, but the outer `if` has no else",
            "  if (outer) {\n    if (choice) {\n      1 => { eat(rows) }\n      else => { eat(rows) }\n    }\n  }\n",
            At::BodyEnd,
            Fate::ReleaseIfLive(ARRAY),
        ),
        (
            "a give before a nested block that returns: the nested `return` sees the give",
            "  eat(rows)\n  if (outer) {\n    return\n  }\n",
            At::Return(0),
            Fate::Omitted(OmitReason::Moved),
        ),
    ];
    for (case, body, at, expected) in cases {
        let source = format!(
            "{}function probe(outer: boolean, inner: boolean, choice: int) -> nothing {{\n  let rows = [1]\n{body}}}\n",
            support::EAT
        );
        let output = clean(&source);
        assert_eq!(
            fate_of(&list_at(&output, "probe", at), "rows"),
            expected,
            "{case}"
        );
    }
}

/// `(user name)` of every `let` of every function whose value the checker records as a view.
fn view_names(output: &CheckOutput) -> Vec<String> {
    let mut lets = Vec::new();
    for item in &output.typed_module.module.items {
        if let Item::Function(function) = item {
            lets_of(&function.body, &mut lets);
        }
    }
    let mut names: Vec<String> = lets
        .into_iter()
        .filter(|(span, _)| output.typed_module.non_owned_reads.is_view(span.0, span.1))
        .map(|(_, decl)| decl.2)
        .collect();
    names.sort();
    names
}

/// Bug: a view that C1f records only for a one-step field read is listed as an owner when it
/// reads through a chain of fields, an element or a map lookup (a double free of what the root
/// still owns), or the view, not the root, founds the class. Every row names the views the
/// program holds, and the roots that must stay the owners.
#[test]
fn a_view_through_a_chain_an_element_or_a_lookup_is_not_an_owner() {
    let shapes = "shape Leaf {\n  label: string\n}\nshape Mid {\n  leaf: Leaf\n  tag: int\n}\nshape Top {\n  mid: Mid\n  count: int\n}\n";
    // (case, body of `probe`, views, roots)
    let cases: [(&str, &str, &[&str], &[&str]); 4] = [
        (
            "two field reads in a row",
            "  let leaf: Leaf = { label: `a` }\n  let mid: Mid = { leaf: leaf, tag: 1 }\n  let top: Top = { mid: mid, count: 2 }\n  let viewed = top.mid\n  let deeper = top.mid.leaf\n  print(deeper.label)\n  print(`${viewed.tag}`)\n",
            &["deeper", "viewed"],
            &["top"],
        ),
        (
            "an element read of an array",
            "  let first: Leaf = { label: `a` }\n  let leaves: array<Leaf> = [first]\n  let picked = leaves[0]\n  print(`${picked.exists()}`)\n",
            &["picked"],
            &["leaves"],
        ),
        (
            "a lookup in a map",
            "  let leaf: Leaf = { label: `a` }\n  let table: map<string, Leaf> = { `k`: leaf }\n  let found = table[`k`]\n  print(`${found.exists()}`)\n",
            &["found"],
            &["table"],
        ),
        (
            "a view whose root is then given away",
            "  let leaf: Leaf = { label: `a` }\n  let mid: Mid = { leaf: leaf, tag: 1 }\n  let seen = mid.leaf\n  print(seen.label)\n  keep(mid)\n",
            &["seen"],
            &[],
        ),
    ];
    for (case, body, views, roots) in cases {
        let source = format!(
            "{shapes}function keep(give mid: Mid) -> nothing {{\n  print(`${{mid.tag}}`)\n}}\nfunction probe() -> nothing {{\n{body}}}\n"
        );
        let output = clean(&source);
        assert_eq!(
            view_names(&output),
            views
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
            "{case}: the checker records exactly these views"
        );
        let list = body_end(&output, "probe");
        for view in views {
            assert_eq!(
                fate_of(&list, view),
                Fate::Omitted(OmitReason::NotOwner),
                "{case}: `{view}` is a view"
            );
        }
        for root in roots {
            assert!(
                fate_of(&list, root).releases(),
                "{case}: `{root}` stays the owner: {list:?}"
            );
        }
        assert_eq!(c1f_agreement(&output), Vec::<String>::new(), "{case}");
    }
}

/// Bug: a string that joins another class is omitted as moved when that class is given (a leak of
/// its bytes: a `let` string owns a copy that the class's give does not take), or a string that
/// is its OWN class's founder is released after its class was given (a double free with the
/// callee). Only the founder of its own class follows the give; a joined string is released by
/// its own scope.
#[test]
fn a_string_joined_to_a_given_class_is_released_but_a_given_string_is_omitted() {
    let prelude = "shape Bucket {\n  text: string\n  rows: array<int>\n}\nfunction consume(give bucket: Bucket) -> nothing {\n  print(bucket.text)\n}\nfunction takes(give text: string) -> nothing {\n  print(text)\n}\n";
    // (case, body of `probe`, edge, string binding, expected fate)
    let cases: [(&str, &str, At, &str, Fate); 4] = [
        (
            "a string read out of a shape that is then given",
            "  let bucket: Bucket = { text: `a`, rows: [1] }\n  let alias = bucket.text\n  if (flag) {\n    consume(bucket)\n    return\n  }\n  print(alias)\n",
            At::Return(0),
            "alias",
            Fate::Release(ReleaseKind::String),
        ),
        (
            "the same string at the end of the body: only the path that kept the shape reaches it",
            "  let bucket: Bucket = { text: `a`, rows: [1] }\n  let alias = bucket.text\n  if (flag) {\n    consume(bucket)\n    return\n  }\n  print(alias)\n",
            At::BodyEnd,
            "alias",
            Fate::ReleaseIfLive(ReleaseKind::String),
        ),
        (
            "a string given on every path is its class's founder: the callee releases it",
            "  let label = `n=${3}`\n  if (flag) {\n    takes(label)\n    return\n  }\n  takes(label)\n",
            At::Return(0),
            "label",
            Fate::Omitted(OmitReason::Moved),
        ),
        (
            "a string given on one path only is released if live",
            "  let label = `n=${3}`\n  if (flag) {\n    takes(label)\n  }\n",
            At::BodyEnd,
            "label",
            Fate::ReleaseIfLive(ReleaseKind::String),
        ),
    ];
    for (case, body, at, name, expected) in cases {
        let source = format!("{prelude}function probe(flag: boolean) -> nothing {{\n{body}}}\n");
        let output = clean(&source);
        assert_eq!(
            fate_of(&list_at(&output, "probe", at), name),
            expected,
            "{case}"
        );
    }
}
