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
        "function plain() -> nothing {\n  let rows = [1, 2, 3]\n  print(rows.count().toString())\n}\n",
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
        "function borrowed(share rows: array<int>) -> nothing {\n  print(rows.count().toString())\n}\nfunction owned(give rows: array<int>) -> nothing {\n  print(rows.count().toString())\n}\n",
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

/// Bug: a reassigned binding's slot may hold a value another binding owns; it must never free.
#[test]
fn a_reassigned_binding_is_never_released() {
    let output = clean(
        "function again() -> nothing {\n  let rows = [1, 2, 3]\n  rows = [4, 5]\n  print(rows.count().toString())\n}\n",
    );
    assert_eq!(
        fate_of(&body_end(&output, "again"), "rows"),
        Fate::Omitted(OmitReason::Reassigned)
    );
}

/// Bug: a generic body is left out of the plan ("every scope"), or its type-parameter values are
/// released as if their type were known.
#[test]
fn a_generic_body_is_covered_and_its_parameter_values_are_omitted() {
    let output = clean(
        "function keep<T>(give value: T) -> T {\n  let spare = [1, 2]\n  print(spare.count().toString())\n  return value\n}\n",
    );
    let list = body_end(&output, "keep");
    assert_eq!(fate_of(&list, "spare"), Fate::Release(ARRAY));
    assert_eq!(
        fate_of(&list, "value"),
        Fate::Omitted(OmitReason::TypeParameter)
    );
}

/// Bug: a list is recorded for the function body but not for a nested block (a loop body's
/// per-iteration locals leak once per iteration).
#[test]
fn a_loop_body_has_its_own_list() {
    let output = clean(
        "function looped() -> nothing {\n  let count = 3\n  while (count > 0) {\n    let row = [1]\n    print(row.count().toString())\n    count = count - 1\n  }\n}\n",
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
        size: None,
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
        (
            Type::Union {
                variants: vec![Type::Int, array.clone()],
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
                size: None,
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
        "shape Label {\n  text: string\n}\nfunction sleeper() -> nothing {\n  let words: fixed<string> = [`a`, `b`]\n  let label: Label = { text: `a` }\n  wait sleep(1)\n}\n",
    );
    assert_eq!(
        crossing_names(&output, "sleeper", Names::User),
        vec!["label".to_string()],
        "the shape crosses (frame backed); `fixed<string>` cannot cross a `wait` yet"
    );
    assert!(ReleaseKind::Shape.has_frame_backing());
    assert!(!ReleaseKind::Fixed.has_frame_backing());
}

// ── The crossing set ────────────────────────────────────────────────────────────────────────

fn crossing_names(output: &CheckOutput, function: &str, names: Names) -> Vec<String> {
    let function = function_of(output, function);
    let params: Vec<&str> = function.params.iter().map(|p| p.name.as_str()).collect();
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
        "function late() -> nothing {\n  wait sleep(1)\n  let rows = [1, 2, 3]\n  print(rows.count().toString())\n}\n",
    );
    assert_eq!(
        crossing_names(&output, "late", Names::User),
        Vec::<String>::new()
    );
}

/// Bug: a `return` after a wait releases the outer owner without a frame slot.
#[test]
fn an_owner_crosses_a_wait_to_reach_a_return_release() {
    let output = clean(
        "function guarded(flag: boolean) -> nothing {\n  let rows = [1, 2, 3]\n  wait sleep(1)\n  if (flag) {\n    return\n  }\n}\n",
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

/// Bug: `let x` twice in one block replaced the first binding in the scope's frame, so the first
/// array was in no list at all (a leak on every path). Both bindings leave scope with the block.
/// Producer: `Scope::insert` dropped the shadowed entry.
#[test]
fn a_binding_shadowed_in_its_own_block_is_still_released() {
    let output = clean(
        "function twice() -> nothing {\n  let x = [1]\n  let x = [2]\n  print(x.count().toString())\n}\n",
    );
    let list = body_end(&output, "twice");
    assert_eq!(
        list.iter().filter(|(name, _)| name == "x").count(),
        2,
        "both `x` bindings are listed: {list:?}"
    );
    assert!(
        list.iter().all(|(_, fate)| *fate == Fate::Release(ARRAY)),
        "{list:?}"
    );
}

/// Bug: a shadowed owner whose class is returned through a class-mate is released anyway (double
/// free with the caller) when the scope forgets it or freezes its state at the shadowing.
#[test]
fn a_shadowed_owner_follows_its_class_when_a_class_mate_is_returned() {
    let output = clean(
        "function pass(share rows: array<int>) -> array<int> {\n  return rows\n}\nfunction shadowed() -> array<int> {\n  let x = [1]\n  let y = pass(x)\n  let x = [9]\n  return y\n}\n",
    );
    let returns = return_edges(&output, "shadowed");
    let list = &returns[0];
    let firsts: Vec<Fate> = list
        .iter()
        .filter(|(name, _)| name == "x")
        .map(|(_, fate)| *fate)
        .collect();
    assert_eq!(firsts.len(), 2, "both `x` are listed: {list:?}");
    assert_eq!(
        firsts.iter().filter(|fate| fate.releases()).count(),
        1,
        "only the unrelated second `x` releases: {list:?}"
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
        "function work(a: array<int>, b: array<int>) -> nothing {\n  print(a.count().toString())\n}\nfunction spawns(flag: boolean) -> nothing {\n  let a: array<int> = [1]\n  let b: array<int> = [2]\n  if (flag) {\n    background work(a, b)\n  }\n}\n",
    );
    let list = body_end(&output, "spawns");
    assert_eq!(fate_of(&list, "a"), Fate::ReleaseIfLive(ARRAY));
    assert_eq!(fate_of(&list, "b"), Fate::ReleaseIfLive(ARRAY));
    let function = function_of(&output, "spawns");
    let mut founders = output
        .typed_module
        .release_plan
        .founders_moved_in(&function.body.stmts[2]);
    founders.sort();
    assert_eq!(founders, vec!["a".to_string(), "b".to_string()]);
}
