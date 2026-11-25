// v0.3-M9 C5 (plan #17): the persisted scope-exit release plan.
//
// WHY (High tier: ownership / data integrity): codegen (C6/C7) frees heap locals from this plan and
// derives nothing itself. A wrong fate is an early or a double free, or a leak. Each test asserts
// the plan's recorded fate at an exit edge, not a program's output: there is no release pass yet,
// so the miss is only observable here. The tests are two tables of NAMED cases, one named test per
// input class: `plan_cases!` (a program through the plan) and `heap_class_cases!` (a type through
// `heap_class_of`), plus the corpus sweep.
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
use ynz_ast::nodes::{Block, Expr, FunctionDecl, Item, Stmt};
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
// bug: the release plan disagrees with the live scope or the transfer record on some fixture or example, so codegen frees a value twice, too early or never | tier: High
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
    let unexercised_fates: Vec<&str> = [
        ("released", &tally.released),
        ("released if live", &tally.released_if_live),
        ("moved", &tally.moved),
        ("returned", &tally.returned),
        ("view", &tally.views),
        ("heap move", &tally.heap_moves),
    ]
    .into_iter()
    .filter(|(_, count)| count.load(Ordering::Relaxed) == 0)
    .map(|(name, _)| name)
    .collect();
    assert!(
        unexercised_fates.is_empty(),
        "the corpus exercised no {unexercised_fates:?} fate over {} edges",
        tally.edges.load(Ordering::Relaxed)
    );
    // Every kind the corpus holds is classified; none is omitted for being unsupported (the
    // parity check rejects an omission without a reason). Print what it exercised on failure.
    let kinds = tally.kinds.lock().expect("mutex").clone();
    let unexercised_kinds: Vec<ReleaseKind> = [
        ReleaseKind::String,
        ReleaseKind::Array,
        ReleaseKind::Map,
        ReleaseKind::Shape,
        ReleaseKind::Maybe,
        ReleaseKind::Union,
    ]
    .into_iter()
    .filter(|expected| !kinds.contains(expected))
    .collect();
    assert!(
        unexercised_kinds.is_empty(),
        "the corpus never exercised {unexercised_kinds:?}; it did {kinds:?}"
    );
    assert!(
        findings.is_empty(),
        "{} findings:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

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

// ── Programs shared by several cases ────────────────────────────────────────────────────────

const PARAMETER_PROGRAM: &str = "function borrowed(share rows: Array<Int>) -> Nothing {\n  print(`${rows.count()}`)\n}\nfunction owned(give rows: Array<Int>) -> Nothing {\n  print(`${rows.count()}`)\n}\n";

const TYPE_PARAMETER_PROGRAM: &str = "function drop<T>(give value: T) -> Nothing {\n  print(`x`)\n}\nfunction maybeKeep<T>(give value: T, flag: Boolean) -> Array<T> {\n  let items: Array<T> = []\n  if (flag) {\n    items.add(value)\n  }\n  return items\n}\n";

const SHAPES_BY_FIELDS_PROGRAM: &str = "shape Label {\n  text: String\n}\nshape Plain {\n  size: Int\n}\nshape Holder {\n  inner: Plain\n}\nfunction risky() -> Int errors {\n  return 1\n}\nfunction kinds() -> Nothing {\n  let label: Label = { text: `a` }\n  let plain: Plain = { size: 1 }\n  let holder: Holder = { inner: { size: 2 } }\n  let words: Fixed<String> = [`a`, `b`]\n  let result = risky()\n  if (result.failed()) {\n    print(`bad`)\n  }\n  print(plain.size.toString())\n  print(label.text)\n  print(holder.inner.size.toString())\n  print(words.count().toString())\n  print(result.toString())\n}\n";

const GIVE_STRING_PROGRAM: &str = "function consume(give text: String) -> Nothing {\n  print(text)\n}\nfunction borrow(share text: String) -> Nothing {\n  print(text)\n}\nfunction caller() -> Nothing {\n  let given = `n=${3}`\n  let lent = `m=${4}`\n  consume(given)\n  borrow(lent)\n}\n";

const GENERIC_AND_RECURSIVE_PROGRAM: &str = "shape Pair<A, B> {\n  first: A\n  second: B\n}\nshape Node {\n  label: String\n  next: Maybe<Node>\n}\nfunction pairs(give numbers: Pair<Int, Int>, give named: Pair<String, Int>, give chain: Node) -> Nothing {\n  print(numbers.first.toString())\n  print(named.first)\n  print(chain.label)\n}\n";

/// `rows` is given at most once, in `body`; each case reads the edge it names.
fn probe_giving_rows(body: &str) -> String {
    format!(
        "{}function probe(outer: Boolean, inner: Boolean, choice: Int) -> Nothing {{\n  let rows = [1]\n{body}}}\n",
        support::EAT
    )
}

/// A program holding views over a leaf, a mid and a top shape, in `probe`'s `body`.
fn view_probe(body: &str) -> String {
    let shapes = "shape Leaf {\n  label: String\n}\nshape Mid {\n  leaf: Leaf\n  tag: Int\n}\nshape Top {\n  mid: Mid\n  count: Int\n}\n";
    format!(
        "{shapes}function keep(give mid: Mid) -> Nothing {{\n  print(`${{mid.tag}}`)\n}}\nfunction probe() -> Nothing {{\n{body}}}\n"
    )
}

/// A program with a string-holding shape and two functions that take a string or the shape, in
/// `probe`'s `body`.
fn string_probe(body: &str) -> String {
    let prelude = "shape Bucket {\n  text: String\n  rows: Array<Int>\n}\nfunction consume(give bucket: Bucket) -> Nothing {\n  print(bucket.text)\n}\nfunction takes(give text: String) -> Nothing {\n  print(text)\n}\n";
    format!("{prelude}function probe(flag: Boolean) -> Nothing {{\n{body}}}\n")
}

/// The view names of `output` are exactly `views`, each is omitted as not an owner, each root
/// stays an owner, and the plan agrees with C1f's records.
fn assert_views_and_roots(output: &CheckOutput, views: &[&str], roots: &[&str]) {
    assert_eq!(
        view_names(output),
        views
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
        "the checker records exactly these views"
    );
    let list = body_end(output, "probe");
    for view in views {
        assert_eq!(
            fate_of(&list, view),
            Fate::Omitted(OmitReason::NotOwner),
            "`{view}` is a view"
        );
    }
    for root in roots {
        assert!(
            fate_of(&list, root).releases(),
            "`{root}` stays the owner: {list:?}"
        );
    }
    assert_eq!(c1f_agreement(output), Vec::<String>::new());
}

fn read_names(stmts: &[Stmt]) -> Vec<String> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Expr(Expr::Ident(name, _)) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// One named test per program: `plan_cases!(name: source, |output| { checks }, "bug it catches";)`
/// checks `source` clean and hands the checked module to the checks.
macro_rules! plan_cases {
    ($($case:ident: $source:expr, |$output:ident| $checks:block, $bug:literal;)+) => {
        $(
            // bug: the release plan gives a binding the wrong fate at an exit edge, or lists, moves or lets cross the wrong owners, so codegen frees a value twice, too early or never | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                let $output = clean(&$source);
                $checks
            }
        )+
    };
}

plan_cases! {
    a_plain_local_is_released_at_the_end_of_its_block: "function plain() -> Nothing {\n  let rows = [1, 2, 3]\n  print(`${rows.count()}`)\n}\n", |output| {
        assert_eq!(
            fate_of(&body_end(&output, "plain"), "rows"),
            Fate::Release(ARRAY)
        );
    }, "Bug: a plain owner is left out of its scope's list, so it leaks.";
    a_binding_given_on_every_path_is_omitted_as_moved: format!(
        "{}function giver() -> Nothing {{\n  let rows = [1, 2, 3]\n  eat(rows)\n}}\n",
        support::EAT
    ), |output| {
        assert_eq!(
            fate_of(&body_end(&output, "giver"), "rows"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: a binding given away on every path is released too (double free with the callee).";
    a_binding_given_on_some_paths_is_released_if_live_and_has_a_move_site: format!(
        "{}function some(flag: Boolean) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n}}\n",
        support::EAT
    ), |output| {
        assert_eq!(
            fate_of(&body_end(&output, "some"), "rows"),
            Fate::ReleaseIfLive(ARRAY)
        );
        let sites: Vec<_> = output.typed_module.release_plan.move_sites().collect();
        assert_eq!(sites.len(), 1, "one read moves `rows`: {sites:?}");
        assert_eq!(sites[0].1.founder.2, "rows");
    }, "Bug: a binding given on SOME paths is released unconditionally (double free on the give path) or never (a leak on the other); it needs the run-time test, and a move site that nulls the slot.";
    an_early_return_before_a_give_still_releases_the_binding: format!(
        "{}function early(flag: Boolean) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    return\n  }}\n  eat(rows)\n}}\n",
        support::EAT
    ), |output| {
        let returns = return_edges(&output, "early");
        assert_eq!(fate_of(&returns[0], "rows"), Fate::Release(ARRAY));
        assert_eq!(
            fate_of(&body_end(&output, "early"), "rows"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: a `return` before the give omits the binding (a leak on that path), or the list at the body end releases it after the give: each edge has its OWN fate, from the path to it.";
    the_returned_binding_is_omitted_at_its_return: "function make() -> Array<Int> {\n  let rows = [1, 2, 3]\n  let spare = [4, 5]\n  return rows\n}\n", |output| {
        let returns = return_edges(&output, "make");
        assert_eq!(
            fate_of(&returns[0], "rows"),
            Fate::Omitted(OmitReason::Returned)
        );
        assert_eq!(fate_of(&returns[0], "spare"), Fate::Release(ARRAY));
    }, "Bug: the returned value is released at the `return` (the caller gets a freed array).";
    a_borrowed_parameter_is_the_callers: PARAMETER_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "borrowed"), "rows"),
            Fate::Omitted(OmitReason::CallerOwns)
        );
    }, "Bug: a parameter the caller owns is released by the callee (a double free with the caller).";
    a_give_parameter_is_released_by_the_callee: PARAMETER_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "owned"), "rows"),
            Fate::Release(ARRAY)
        );
    }, "Bug: a `give` parameter is not released, because the callee owns it (a leak).";
    a_reassigned_owner_is_released_and_its_assignment_releases_the_old_value: "function again() -> Nothing {\n  let rows = [1, 2, 3]\n  rows = [4, 5]\n  print(`${rows.count()}`)\n}\n", |output| {
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
    }, "Bug: a reassigned owner leaks the value each assignment replaces (`Omitted(Reassigned)`), or its assignment frees a value it no longer owns: the plan releases it at its exit and records the release its assignment makes.";
    a_generic_body_is_covered_and_its_parameter_values_are_planned: "function keep<T>(give value: T) -> T {\n  let spare = [1, 2]\n  print(`${spare.count()}`)\n  return value\n}\n", |output| {
        let list = body_end(&output, "keep");
        assert_eq!(fate_of(&list, "spare"), Fate::Release(ARRAY));
        // The value is returned: the caller owns it from then on, whatever `T` is.
        assert_eq!(
            fate_of(&return_edges(&output, "keep")[0], "value"),
            Fate::Omitted(OmitReason::Returned)
        );
    }, "Bug: a generic body is left out of the plan (\"every scope\"), or its type-parameter values are dropped from it (a given `T` leaked in every instance).";
    a_given_type_parameter_value_nothing_hands_on_is_released: TYPE_PARAMETER_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "drop"), "value"),
            Fate::Release(ReleaseKind::TypeParameter)
        );
    }, "Bug (leak): a given `T` that nothing hands on was omitted as a type parameter, so every instance leaked it; it is a releasing fate of `ReleaseKind::TypeParameter`, which codegen resolves by the instance's type.";
    a_given_type_parameter_value_stored_on_some_paths_is_released_if_live: TYPE_PARAMETER_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "maybeKeep"), "value"),
            Fate::ReleaseIfLive(ReleaseKind::TypeParameter)
        );
    }, "Bug: a given `T` stored on some paths only is released unconditionally (a double free on the store path) or never (a leak on the other): it is released only if live.";
    a_loop_body_has_its_own_list: "function looped() -> Nothing {\n  let count = 3\n  while (count > 0) {\n    let row = [1]\n    print(`${row.count()}`)\n    count = count - 1\n  }\n}\n", |output| {
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
    }, "Bug: a list is recorded for the function body but not for a nested block (a loop body's per-iteration locals leak once per iteration).";
    an_auto_propagation_omits_the_propagated_binding_as_returned: "function risky() -> Int errors {\n  return 1\n}\nfunction chained() -> Int errors {\n  let rows = [1, 2]\n  let value = risky()\n  return value\n}\n", |output| {
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
    }, "Bug: the propagation edge is missing, so an errors function that returns early on a failed call leaks every owner it holds; and the edge lists the failed binding as `Release` (the error object is freed and returned: a double free), so it must omit it as returned and the transfer record must say it escapes by return. Producer: `Checker::resolve_ident` recorded the propagation edge with no returned classes.";
    a_string_is_released_if_live_exactly_when_the_record_lists_a_read: "function words() -> Nothing {\n  let unused = `plain`\n  let label = `n=${3}`\n  print(label)\n}\n", |output| {
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
    }, "Bug: a string with no read after it is treated as moved (leaks) or a last-use read that a store may take is released unconditionally (double free with the store's owner).";
    a_shape_with_a_string_field_owns_heap: SHAPES_BY_FIELDS_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "kinds"), "label"),
            Fate::Release(ReleaseKind::Shape)
        );
    }, "Bug: a shape's heap is judged by its name or its first field: a shape with a string owns heap, so it is released.";
    a_shape_holding_a_shape_owns_that_shapes_cell: SHAPES_BY_FIELDS_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "kinds"), "holder"),
            Fate::Release(ReleaseKind::NestedShape)
        );
    }, "Bug: a shape's heap is judged by its name or its first field: a shape holding a shape owns that shape's cell.";
    a_shape_of_ints_owns_no_heap_and_is_not_listed: SHAPES_BY_FIELDS_PROGRAM, |output| {
        let list = body_end(&output, "kinds");
        assert!(
            list.iter().all(|(name, _)| name != "plain"),
            "a shape of ints owns no heap, so it is not listed: {list:?}"
        );
    }, "Bug: a shape's heap is judged by its name or its first field: a shape of ints owns nothing, so it must not be listed (a release of a value type).";
    a_fixed_list_of_strings_is_released_as_a_fixed_list: SHAPES_BY_FIELDS_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "kinds"), "words"),
            Fate::Release(ReleaseKind::Fixed)
        );
    }, "Bug: a `Fixed<String>` list owns heap but is left out of the plan (a leak).";
    an_errors_capable_result_is_released_as_an_errors_capable_value: SHAPES_BY_FIELDS_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "kinds"), "result"),
            Fate::Release(ReleaseKind::ErrorsCapable)
        );
    }, "Bug: an errors-capable result owns an error object but is left out of the plan (a leak).";
    only_a_kind_with_frame_backing_feeds_the_crossing_set: "shape Label {\n  text: String\n}\nfunction sleeper() -> Nothing {\n  let words: Fixed<String> = [`a`, `b`]\n  let label: Label = { text: `a` }\n  let rows = [1, 2]\n  wait sleep(1)\n}\n", |output| {
        assert_eq!(
            crossing_names(&output, "sleeper", Names::User),
            vec!["label".to_string(), "rows".to_string(), "words".to_string()],
            "the array, the flat shape and the `Fixed` list cross (each has a frame home)"
        );
        assert!(ReleaseKind::Array.has_frame_backing());
        assert!(ReleaseKind::Shape.has_frame_backing());
        assert!(ReleaseKind::Fixed.has_frame_backing());
        assert!(!ReleaseKind::Dynamic.has_frame_backing());
    }, "Bug: reading a kind with no frame slot across a suspension as a release rejects programs that compile today (Check 2/2b); only a kind with frame backing feeds the crossing set.";
    an_owner_declared_before_a_wait_crosses_it: "function sleeper() -> Nothing {\n  let rows = [1, 2, 3]\n  wait sleep(1)\n}\n", |output| {
        assert_eq!(
            crossing_names(&output, "sleeper", Names::User),
            vec!["rows".to_string()]
        );
    }, "Bug: an owner declared before a `wait` and released after it has no frame slot, so the release frees a stack address. Not crossing here would be the S3 probe `e1`.";
    an_owner_declared_after_the_last_wait_does_not_cross: "function late() -> Nothing {\n  wait sleep(1)\n  let rows = [1, 2, 3]\n  print(`${rows.count()}`)\n}\n", |output| {
        assert_eq!(
            crossing_names(&output, "late", Names::User),
            Vec::<String>::new()
        );
    }, "Bug: the crossing set is widened to every owner (an extra frame slot and flush per owner, above hand-written cost): an owner no suspension separates from its release must not cross.";
    an_owner_crosses_a_wait_to_reach_a_return_release: format!(
        "{}function guarded(choice: Int) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (choice) {{\n    1 => {{ eat(rows) }}\n    else => {{\n      wait sleep(1)\n      return\n    }}\n  }}\n}}\n",
        support::EAT
    ), |output| {
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
    }, "Bug: a `return` after a wait releases the outer owner without a frame slot. The other arm gives the owner away and the returning arm never reaches the body end, so the `return` edge is the ONLY release that follows the wait: the body-end read alone would not make the owner cross.";
    a_statement_holding_a_move_site_names_its_founder_as_written: format!(
        "{}function some(flag: Boolean) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n  print(`done`)\n}}\n",
        support::EAT
    ), |output| {
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
    }, "Bug: a move that nulls the founder's slot is not seen as a write to it, so the state machine never flushes the nulled slot and a resumed release frees the moved value (S3 probe `c2`).";
    a_binding_shadowed_in_its_own_block_is_still_released: "function twice() -> Nothing {\n  let rows = [1]\n  let rows = [2]\n  print(`${rows.count()}`)\n}\n", |output| {
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
    }, "Bug: `let rows` twice in one block replaced the first binding in the scope's frame, so the first array was in no list at all (a leak on every path). Both bindings leave scope with the block. Producer: `Scope::insert` dropped the shadowed entry.";
    a_shadowed_owner_follows_its_class_when_a_class_mate_is_returned: "function pass<T>(share rows: T) -> T {\n  return rows\n}\nfunction shadowed() -> Array<Int> {\n  let rows = [1]\n  let passed = pass(rows)\n  let rows = [9]\n  return passed\n}\n", |output| {
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
    }, "Bug: a shadowed owner is judged by the wrong binding of its name (the scope forgets it or freezes its state at the shadowing). `return passed` copies `passed` at the `return` (a string, array, map or shape the return does not move is copied), so the caller holds a value of its own and both `rows` are released; before the copy-owner change the return marked the whole class returned.";
    a_spawn_that_gives_two_bindings_moves_both_founders: "function work(first: Array<Int>, second: Array<Int>) -> Nothing {\n  print(`${first.count()}`)\n}\nfunction spawns(flag: Boolean) -> Nothing {\n  let first: Array<Int> = [1]\n  let second: Array<Int> = [2]\n  if (flag) {\n    background work(first, second)\n  }\n}\n", |output| {
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
    }, "Bug: one `background` spawn that gives two bindings is one read; the plan kept the first founder only, so the second (released if live) had no move site and the module failed with an internal error (High: data integrity). Producer: `note_move_site` keyed sites by span alone.";
    a_string_is_released_if_live_only_at_edges_a_listed_read_can_precede: "function late(flag: Boolean) -> Nothing {\n  let label = `n=${3}`\n  if (flag) {\n    return\n  }\n  print(label)\n}\n", |output| {
        assert_eq!(
            fate_of(&return_edges(&output, "late")[0], "label"),
            Fate::Release(ReleaseKind::String),
            "no listed read comes before this `return`"
        );
        assert_eq!(
            fate_of(&body_end(&output, "late"), "label"),
            Fate::ReleaseIfLive(ReleaseKind::String)
        );
    }, "Bug: a string's `ReleaseIfLive` is decided by whether the record lists any read of it, not by whether one can precede THIS edge: the early `return` then frees through a run-time test the path never needs, and (reversed) a read before the edge is missed and the store's owner and the scope both free the string.";
    a_string_parameter_is_not_released_by_the_callee: "function show(text: String) -> Nothing {\n  print(text)\n}\n", |output| {
        assert_eq!(
            fate_of(&body_end(&output, "show"), "text"),
            Fate::Omitted(OmitReason::NotOwner)
        );
    }, "Bug: a string parameter is released by the callee (the caller still owns it: a double free).";
    a_give_string_parameter_is_released_by_the_callee: GIVE_STRING_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "consume"), "text"),
            Fate::Release(ReleaseKind::String)
        );
    }, "Bug: a string given to a `give` parameter is released by both sides (a double free) or by neither (a leak). The callee owns what it was given and releases it.";
    a_share_string_parameter_stays_the_callers: GIVE_STRING_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "borrow"), "text"),
            Fate::Omitted(OmitReason::NotOwner)
        );
    }, "Bug: a `share` string parameter is released by the callee (the caller still owns it: a double free).";
    the_caller_omits_a_string_it_gave_and_releases_one_it_lent: GIVE_STRING_PROGRAM, |output| {
        let caller = body_end(&output, "caller");
        assert_eq!(fate_of(&caller, "given"), Fate::Omitted(OmitReason::Moved));
        assert!(fate_of(&caller, "lent").releases());
    }, "Bug: a string given to a `give` parameter is released by both sides (a double free) or by neither (a leak). The checker refuses a read of it after the call, so the caller omits it as moved; a string it only lent it still owns and releases.";
    a_generic_call_result_is_an_owner_of_its_own_value: "function pass<T>(share rows: T) -> T {\n  return rows\n}\nfunction mate() -> Nothing {\n  let first = [1]\n  let second = pass(first)\n  print(`${second.count()}`)\n}\n", |output| {
        let list = body_end(&output, "mate");
        assert_eq!(fate_of(&list, "first"), Fate::Release(ARRAY));
        assert_eq!(fate_of(&list, "second"), Fate::Release(ARRAY));
    }, "A call result is an owner of its own value, whatever the callee's declared type: a generic callee's `return` copies a heap-typed result at its instance, so `second` holds an array of its own and releases it (as `first` releases its own); the class `first` founds does not reach it. Bug (leak): the result was judged a piece of `first` (`MayAlias`, the type parameter unresolved) and neither bound by move nor released.";
    a_read_that_outlives_its_root_is_a_copy_released_at_both_scope_ends: "shape Bucket {\n  rows: Array<Int>\n}\nfunction make() -> Bucket {\n  let bucket: Bucket = { rows: [9] }\n  return bucket\n}\nfunction under(condition: Boolean) -> Int {\n  let cell: Array<Int> = make().rows\n  if (condition) {\n    let owner: Bucket = make()\n    cell = owner.rows\n  }\n  return cell.count()\n}\n", |output| {
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
    }, "Bug: a binding that outlives the owner of the field it reads (`cell = owner.rows`, `owner` dies at the end of the `if`) is left pointing at what the scope exit frees (a use after free), or both are left unreleased (a leak). The read is a copy that outlives its root, so `cell` owns it and both are released at their own scope exits (decided: Chief Architect, replacing the run-time drop-flag ruling for `OutlivedByNonOwner`; `transfer_record.rs` pins the no-static-founder case that is left).";
    a_return_lists_the_innermost_frame_first_and_the_last_declared_first: "function ordered(flag: Boolean) -> Nothing {\n  let outer_first = [1]\n  let outer_second = [2]\n  if (flag) {\n    let inner_first = [3]\n    let inner_second = [4]\n    return\n  }\n  print(`${outer_first.count()}`)\n  print(`${outer_second.count()}`)\n}\n", |output| {
        let names: Vec<String> = return_edges(&output, "ordered")[0]
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        assert_eq!(
            names,
            vec!["inner_second", "inner_first", "outer_second", "outer_first"]
        );
    }, "Bug: a `return` lists the frames outermost first, or each frame first-declared first: codegen releases in list order, so a later local would be freed after what it borrows from.";
    a_generic_pair_of_ints_owns_no_heap: GENERIC_AND_RECURSIVE_PROGRAM, |output| {
        let list = body_end(&output, "pairs");
        assert!(
            list.iter().all(|(name, _)| name != "numbers"),
            "a pair of ints owns no heap: {list:?}"
        );
    }, "Bug: a generic shape instance is judged by its declaration, not its arguments, or a shape that holds itself recurses forever or is classed as owning nothing: `Pair<Int, Int>` listed.";
    a_generic_pair_holding_a_string_owns_heap: GENERIC_AND_RECURSIVE_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "pairs"), "named"),
            Fate::Release(ReleaseKind::Shape)
        );
    }, "Bug: a generic shape instance is judged by its declaration, not its arguments, or a shape that holds itself recurses forever or is classed as owning nothing: `Pair<String, Int>` not listed.";
    a_recursive_shape_owns_heap: GENERIC_AND_RECURSIVE_PROGRAM, |output| {
        assert_eq!(
            fate_of(&body_end(&output, "pairs"), "chain"),
            Fate::Release(ReleaseKind::Shape)
        );
    }, "Bug: a generic shape instance is judged by its declaration, not its arguments, or a shape that holds itself recurses forever or is classed as owning nothing: a recursive shape owns heap (its label, its boxed link).";
    an_owner_given_on_every_path_does_not_cross_a_wait: format!(
        "{}function moved() -> Nothing {{\n  let rows = [1, 2, 3]\n  eat(rows)\n  wait sleep(1)\n}}\n",
        support::EAT
    ), |output| {
        assert_eq!(
            crossing_names(&output, "moved", Names::User),
            Vec::<String>::new(),
            "given on every path: no release reads it after the wait"
        );
    }, "Bug: an owner omitted from every edge is still read there, so it gets a frame slot and a flush for nothing (above hand-written cost); a binding released only if live is missed and its slot is not preserved across the wait.";
    an_owner_released_if_live_after_a_wait_crosses_it: format!(
        "{}function if_live(flag: Boolean) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    eat(rows)\n  }}\n  wait sleep(1)\n}}\n",
        support::EAT
    ), |output| {
        assert_eq!(
            crossing_names(&output, "if_live", Names::User),
            vec!["rows".to_string()],
            "released if live after the wait"
        );
    }, "Bug: an owner omitted from every edge is still read there, so it gets a frame slot and a flush for nothing (above hand-written cost); a binding released only if live is missed and its slot is not preserved across the wait.";
    a_borrowed_parameter_does_not_cross_a_wait: "function borrowed(share rows: Array<Int>) -> Nothing {\n  wait sleep(1)\n}\n", |output| {
        assert_eq!(
            crossing_names(&output, "borrowed", Names::User),
            Vec::<String>::new(),
            "the caller owns it: no release"
        );
    }, "Bug: an owner omitted from every edge is still read there, so it gets a frame slot and a flush for nothing (above hand-written cost); a binding released only if live is missed and its slot is not preserved across the wait.";
    with_reads_adds_one_read_per_owner_at_each_edge_it_belongs_to: format!(
        "{}function placed(flag: Boolean) -> Nothing {{\n  let rows = [1, 2, 3]\n  if (flag) {{\n    let inner = [4]\n    return\n  }}\n  print(`${{rows.count()}}`)\n}}\n",
        support::EAT
    ), |output| {
        let function = function_of(&output, "placed");
        let reads = output
            .typed_module
            .release_plan
            .reads("placed", Names::User);
        let rewritten = reads.with_reads(&function.body.stmts);
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
    }, "Bug: the reads a release adds are placed at the wrong statement (after the wrong `return`, in the wrong block, or twice), so the one crossing producer sees a use before a suspension it follows, or after one it precedes. `with_reads` is the only place the placement is decided.";
    with_reads_leaves_a_function_with_no_release_reads_unchanged: "function quiet() -> Nothing {\n  print(`x`)\n}\n", |output| {
        let plain = function_of(&output, "quiet");
        assert_eq!(
            output
                .typed_module
                .release_plan
                .reads("quiet", Names::User)
                .with_reads(&plain.body.stmts)
                .len(),
            plain.body.stmts.len()
        );
    }, "Bug: a function with no release reads is rewritten (a statement added or moved) when `with_reads` has nothing to place.";
    with_reads_puts_a_propagation_read_after_its_statement_in_its_own_block: "function risky() -> Int errors {\n  return 1\n}\nfunction chained(flag: Boolean) -> Int errors {\n  let rows = [1, 2]\n  if (flag) {\n    let inner = [3]\n    let value = risky()\n    print(value.toString())\n  }\n  return 0\n}\n", |output| {
        let function = function_of(&output, "chained");
        let rewritten = output
            .typed_module
            .release_plan
            .reads("chained", Names::User)
            .with_reads(&function.body.stmts);
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
    }, "Bug: the read a propagation edge adds is placed at the outer statement as well as inside the nested block that holds the identifier (twice), or in the outer block only (after the nested block's own end, where its locals are already gone). The edge belongs right after the statement that holds the identifier, in the innermost block.";
    a_binding_moved_by_a_store_is_omitted_and_the_new_owner_releases: "shape Bucket {\n  rows: Array<Int>\n}\nfunction move() -> Nothing {\n  let holder: Bucket = { rows: [1, 2] }\n  let moved = holder\n  let total = moved.rows.count()\n  print(`${total}`)\n}\n", |output| {
        assert!(
            !output.typed_module.heap_moves.is_empty(),
            "the program is meant to hold a heap move"
        );
        let list = body_end(&output, "move");
        assert_eq!(fate_of(&list, "holder"), Fate::Omitted(OmitReason::Moved));
        assert!(fate_of(&list, "moved").releases());
        assert_eq!(c1f_agreement(&output), Vec::<String>::new());
    }, "Bug: a binding a store moves (`heap_moves`) is also released at its scope exit (a double free of its boxes): the store owns it, so the moved binding is omitted and the new binding releases.";
    a_name_declared_twice_in_a_function_feeds_no_release_read_to_the_guards: "function twice(flag: Boolean) -> Nothing {\n  let numbers = [1]\n  if (flag) {\n    let reading: Maybe<Int> = numbers.get(0)\n    print(`${reading.or(0)}`)\n  }\n  if (flag) {\n    let reading = [1, 2]\n    wait sleep(1)\n  }\n}\n", |output| {
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
    }, "Bug (a regression of a program that compiled before C5): the release read of an array owner `reading` in one block makes the NAME `reading` cross a `wait`, and the guards judge the name by the first `let v` of the function, a `Maybe<Int>` that does not cross: the program is rejected with \"a `Maybe<Int>` value cannot yet cross a `wait`\". Producer: `resolve_function` fed an owner's user name to the name-based crossing producer when the function declares that name twice.";
    a_give_in_an_inner_if_still_shows_at_a_return_inside_the_outer_if: probe_giving_rows("  if (outer) {\n    if (inner) {\n      eat(rows)\n    }\n    return\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::Return(0)), "rows"),
            Fate::ReleaseIfLive(ARRAY)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: a nested if, edge after the inner join, still inside the outer if.";
    a_give_in_an_inner_if_never_reaches_the_body_end_when_the_outer_branch_returns: probe_giving_rows("  if (outer) {\n    if (inner) {\n      eat(rows)\n    }\n    return\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "rows"),
            Fate::Release(ARRAY)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: a nested if whose outer branch always returns: the give never reaches the body end.";
    a_give_in_one_match_arm_is_released_if_live_at_the_join: probe_giving_rows("  if (choice) {\n    1 => { eat(rows) }\n    2 => { print(`two`) }\n    else => { print(`other`) }\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "rows"),
            Fate::ReleaseIfLive(ARRAY)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: a give in a match arm: some arms give, the others keep it.";
    a_give_on_every_arm_of_a_match_is_omitted_as_moved: probe_giving_rows("  if (choice) {\n    1 => { eat(rows) }\n    else => { eat(rows) }\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "rows"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: the join of two arms decides \"given on every path\" with `any` for `all`: a give on both arms is released if live (a needless run-time test).";
    a_match_arm_that_returns_sees_the_array_still_owned_at_its_return: probe_giving_rows("  if (choice) {\n    1 => { eat(rows) }\n    else => { return }\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::Return(0)), "rows"),
            Fate::Release(ARRAY)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: a match arm that returns sees the array still owned at its `return` (the returning arm still owns the array).";
    a_match_arm_that_returns_never_reaches_the_join: probe_giving_rows("  if (choice) {\n    1 => { eat(rows) }\n    else => { return }\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "rows"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: an arm that returns counted as a path makes the join look \"some paths\" when the only path that reaches it gave.";
    an_inner_match_that_gives_on_every_arm_inside_an_if_without_else_is_released_if_live: probe_giving_rows("  if (outer) {\n    if (choice) {\n      1 => { eat(rows) }\n      else => { eat(rows) }\n    }\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "rows"),
            Fate::ReleaseIfLive(ARRAY)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: every arm of an inner match gives, but the outer `if` has no else.";
    a_give_before_a_nested_block_that_returns_is_seen_by_that_return: probe_giving_rows("  eat(rows)\n  if (outer) {\n    return\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::Return(0)), "rows"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: the fate of a give depends on WHERE it sits, and a construct that hides the give from the join turns an early or a double free into a leak or the reverse: a give before a nested block that returns: the nested `return` sees the give.";
    two_field_reads_in_a_row_are_views_and_the_root_stays_the_owner: view_probe("  let leaf: Leaf = { label: `a` }\n  let mid: Mid = { leaf: leaf, tag: 1 }\n  let top: Top = { mid: mid, count: 2 }\n  let viewed = top.mid\n  let deeper = top.mid.leaf\n  print(deeper.label)\n  print(`${viewed.tag}`)\n"), |output| {
        assert_views_and_roots(&output, &["deeper", "viewed"], &["top"]);
    }, "Bug: a view that C1f records only for a one-step field read is listed as an owner when it reads through a chain of fields, an element or a map lookup (a double free of what the root still owns), or the view, not the root, founds the class: two field reads in a row.";
    an_element_read_of_an_array_is_a_view: view_probe("  let first: Leaf = { label: `a` }\n  let leaves: Array<Leaf> = [first]\n  let picked = leaves[0]\n  print(`${picked.exists()}`)\n"), |output| {
        assert_views_and_roots(&output, &["picked"], &["leaves"]);
    }, "Bug: a view that C1f records only for a one-step field read is listed as an owner when it reads through a chain of fields, an element or a map lookup (a double free of what the root still owns), or the view, not the root, founds the class: an element read of an array.";
    a_lookup_in_a_map_is_a_view: view_probe("  let leaf: Leaf = { label: `a` }\n  let table: Map<String, Leaf> = { `k`: leaf }\n  let found = table[`k`]\n  print(`${found.exists()}`)\n"), |output| {
        assert_views_and_roots(&output, &["found"], &["table"]);
    }, "Bug: a view that C1f records only for a one-step field read is listed as an owner when it reads through a chain of fields, an element or a map lookup (a double free of what the root still owns), or the view, not the root, founds the class: a lookup in a map.";
    a_view_whose_root_is_then_given_away_is_still_a_view: view_probe("  let leaf: Leaf = { label: `a` }\n  let mid: Mid = { leaf: leaf, tag: 1 }\n  let seen = mid.leaf\n  print(seen.label)\n  keep(mid)\n"), |output| {
        assert_views_and_roots(&output, &["seen"], &[]);
    }, "Bug: a view that C1f records only for a one-step field read is listed as an owner when it reads through a chain of fields, an element or a map lookup (a double free of what the root still owns), or the view, not the root, founds the class: a view whose root is then given away.";
    a_string_read_out_of_a_given_shape_is_released_at_the_return_that_gives_it: string_probe("  let bucket: Bucket = { text: `a`, rows: [1] }\n  let alias = bucket.text\n  if (flag) {\n    consume(bucket)\n    return\n  }\n  print(alias)\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::Return(0)), "alias"),
            Fate::Release(ReleaseKind::String)
        );
    }, "Bug: a string that joins another class is omitted as moved when that class is given (a leak of its bytes), or a string that is its OWN class's founder is released after its class was given (a double free with the callee); only the founder of its own class follows the give: a string read out of a shape that is then given.";
    a_string_read_out_of_a_shape_is_released_if_live_at_the_body_end: string_probe("  let bucket: Bucket = { text: `a`, rows: [1] }\n  let alias = bucket.text\n  if (flag) {\n    consume(bucket)\n    return\n  }\n  print(alias)\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "alias"),
            Fate::ReleaseIfLive(ReleaseKind::String)
        );
    }, "Bug: a string that joins another class is omitted as moved when that class is given (a leak of its bytes), or a string that is its OWN class's founder is released after its class was given (a double free with the callee); only the founder of its own class follows the give: the same string at the end of the body: only the path that kept the shape reaches it.";
    a_string_given_on_every_path_is_omitted_as_moved: string_probe("  let label = `n=${3}`\n  if (flag) {\n    takes(label)\n    return\n  }\n  takes(label)\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::Return(0)), "label"),
            Fate::Omitted(OmitReason::Moved)
        );
    }, "Bug: a string that joins another class is omitted as moved when that class is given (a leak of its bytes), or a string that is its OWN class's founder is released after its class was given (a double free with the callee); only the founder of its own class follows the give: a string given on every path is its class's founder: the callee releases it.";
    a_string_given_on_one_path_only_is_released_if_live: string_probe("  let label = `n=${3}`\n  if (flag) {\n    takes(label)\n  }\n"), |output| {
        assert_eq!(
            fate_of(&list_at(&output, "probe", At::BodyEnd), "label"),
            Fate::ReleaseIfLive(ReleaseKind::String)
        );
    }, "Bug: a string that joins another class is omitted as moved when that class is given (a leak of its bytes), or a string that is its OWN class's founder is released after its class was given (a double free with the callee); only the founder of its own class follows the give: a string given on one path only is released if live.";
}

// ── What a type needs ───────────────────────────────────────────────────────────────────────

fn heap_class_in_empty_tables(ty: &Type) -> HeapClass {
    let shapes = ShapeTable::default();
    let generic_shapes = GenericShapeTable::default();
    let tables = TypeTables {
        shapes: &shapes,
        generic_shapes: &generic_shapes,
    };
    heap_class_of(ty, &tables)
}

fn array_of_ints() -> Type {
    Type::BuiltinArray {
        elem: Box::new(Type::Int),
    }
}

fn type_parameter(name: &str) -> Type {
    Type::TypeParam { name: name.into() }
}

fn channel_of_ints() -> Type {
    Type::BuiltinChannel {
        elem: Box::new(Type::Int),
    }
}

/// One named test per type: `heap_class_cases!(name: type => class, "bug it catches";)`.
macro_rules! heap_class_cases {
    ($($case:ident: $type:expr => $expected:expr, $bug:literal;)+) => {
        $(
            // bug: heap_class_of classifies a type that owns heap as owning none (a leak for good: codegen has nothing to release it from) or a value type as owning heap, so a binding is freed wrongly or never | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_eq!(heap_class_in_empty_tables(&$type), $expected);
            }
        )+
    };
}

heap_class_cases! {
    an_int_owns_no_heap: Type::Int => HeapClass::NoHeap, "a value type is listed";
    a_bool_owns_no_heap: Type::Bool => HeapClass::NoHeap, "a value type is listed";
    a_string_is_a_heap_string: Type::String => HeapClass::Heap(ReleaseKind::String), "a string is classified as owning none";
    an_array_is_a_heap_array: array_of_ints() => HeapClass::Heap(ARRAY), "an array is classified as owning none";
    a_map_is_a_heap_map: Type::BuiltinMap {
        key: Box::new(Type::Int),
        val: Box::new(Type::Int),
    } => HeapClass::Heap(ReleaseKind::Map), "a map is classified as owning none";
    a_maybe_of_an_int_owns_no_heap: Type::Maybe {
        inner: Box::new(Type::Int),
    } => HeapClass::NoHeap, "a maybe holding a value type is listed";
    a_maybe_of_an_array_is_a_heap_maybe: Type::Maybe {
        inner: Box::new(array_of_ints()),
    } => HeapClass::Heap(ReleaseKind::Maybe), "a maybe holding heap is classified as owning none";
    // A union with no shape variant and one variant beside `Nothing` is that variant's raw
    // pointer (`is_envelope_union`): the slot holds what its one heap variant holds.
    a_union_of_nothing_and_one_heap_variant_is_that_variants_pointer: Type::Union {
        variants: vec![Type::Nothing, array_of_ints()],
    } => HeapClass::Heap(ARRAY), "the slot of an envelope union is released by the wrong kind";
    // With two variants beside `Nothing`, one of them heap, the slot is tagged and releases by
    // the variant its tag names.
    a_union_of_an_int_and_an_array_is_a_tagged_union: Type::Union {
        variants: vec![Type::Int, array_of_ints()],
    } => HeapClass::Heap(ReleaseKind::Union), "a tagged union with a heap variant is classified as owning none";
    a_union_of_a_string_and_an_array_is_a_tagged_union: Type::Union {
        variants: vec![Type::String, array_of_ints()],
    } => HeapClass::Heap(ReleaseKind::Union), "a tagged union of two heap variants is classified as owning none";
    a_union_of_a_string_and_a_channel_is_a_tagged_union: Type::Union {
        variants: vec![Type::String, channel_of_ints()],
    } => HeapClass::Heap(ReleaseKind::Union), "a tagged union holding a channel is classified as owning none";
    a_union_of_value_types_owns_no_heap: Type::Union {
        variants: vec![Type::Int, Type::Bool],
    } => HeapClass::NoHeap, "a union of value types is listed";
    a_fixed_list_of_strings_is_a_heap_fixed_list: Type::BuiltinFixed {
        elem: Box::new(Type::String),
        size: ynz_typeck::FixedLength::Unknown,
    } => HeapClass::Heap(ReleaseKind::Fixed), "a fixed list of heap elements is classified as owning none";
    a_fixed_list_of_ints_owns_no_heap: Type::BuiltinFixed {
        elem: Box::new(Type::Int),
        size: ynz_typeck::FixedLength::Unknown,
    } => HeapClass::NoHeap, "a fixed list of value types is listed";
    a_dynamic_contract_value_is_heap: Type::Dynamic {
        contract: "Drawable".into(),
    } => HeapClass::Heap(ReleaseKind::Dynamic), "a dynamic contract value is classified as owning none";
    an_errors_capable_value_is_heap: Type::ErrorsCapable {
        inner: Box::new(Type::Int),
    } => HeapClass::Heap(ReleaseKind::ErrorsCapable), "an errors-capable value is classified as owning none";
    a_maybe_of_a_type_parameter_is_decided_by_the_instance: Type::Maybe {
        inner: Box::new(type_parameter("T")),
    } => HeapClass::TypeParameter, "a maybe of a type parameter is judged without its instance";
    a_type_parameter_is_decided_by_the_instance: type_parameter("T") => HeapClass::TypeParameter, "a type parameter is judged without its instance";
    // A shape the tables do not describe is assumed to own heap: leaking is the wrong error.
    an_undescribed_shape_is_assumed_to_own_heap: Type::Shape {
        name: "Unknown".into(),
    } => HeapClass::Heap(ReleaseKind::NestedShape), "a shape the tables do not describe is assumed to own nothing (a leak)";
    a_channel_is_a_heap_channel: channel_of_ints() => HeapClass::Heap(ReleaseKind::Channel), "a channel is classified as owning none";
    a_background_handle_is_a_heap_task_handle: Type::BackgroundHandle {
        result: Box::new(Type::Int),
        msg_elem: None,
    } => HeapClass::Heap(ReleaseKind::TaskHandle), "a background task handle is classified as owning none";
    a_sensitive_string_is_a_heap_string: Type::Sensitive {
        inner: Box::new(Type::String),
    } => HeapClass::Heap(ReleaseKind::String), "a sensitive string is classified as owning none";
    a_sensitive_int_owns_no_heap: Type::Sensitive {
        inner: Box::new(Type::Int),
    } => HeapClass::NoHeap, "a sensitive value type is listed";
    a_map_entry_holding_a_string_is_heap: Type::MapEntry {
        key: Box::new(Type::Int),
        val: Box::new(Type::String),
    } => HeapClass::Heap(ReleaseKind::MapEntry), "a map entry holding heap is classified as owning none";
    a_map_entry_of_ints_owns_no_heap: Type::MapEntry {
        key: Box::new(Type::Int),
        val: Box::new(Type::Int),
    } => HeapClass::NoHeap, "a map entry of value types is listed";
    a_map_entry_of_a_type_parameter_is_decided_by_the_instance: Type::MapEntry {
        key: Box::new(Type::Int),
        val: Box::new(type_parameter("V")),
    } => HeapClass::TypeParameter, "a map entry of a type parameter is judged without its instance";
    a_union_of_an_int_and_a_type_parameter_is_decided_by_the_instance: Type::Union {
        variants: vec![Type::Int, type_parameter("T")],
    } => HeapClass::TypeParameter, "a union holding a type parameter is judged without its instance";
    // One heap-owning variant decides, even beside a type parameter.
    a_union_of_a_type_parameter_and_an_array_is_a_tagged_union: Type::Union {
        variants: vec![type_parameter("T"), array_of_ints()],
    } => HeapClass::Heap(ReleaseKind::Union), "a heap variant beside a type parameter is ignored";
    a_fixed_list_of_a_type_parameter_is_decided_by_the_instance: Type::BuiltinFixed {
        elem: Box::new(type_parameter("T")),
        size: ynz_typeck::FixedLength::Unknown,
    } => HeapClass::TypeParameter, "a fixed list of a type parameter is judged without its instance";
}
