// WHY: the `parallel_groups` inlay hint and the codegen spawn set MUST always agree — the
// registry promise on this domain is "the same set that drives codegen routing, so the hint and
// the binary always agree". This suite locks that invariant by asserting the hint set equals the
// admission decision both crates read (`admitted_cpu_group`). If a future edit drives the hint
// from the broad parallelizable set again (instead of the admitted set), the IDE would claim a
// group runs on a separate core that codegen never spawned — these tests catch that immediately.
//
// The codegen binary-side proof (IR-grep `ynz_rt_spawn_blocking_joinable` == 0 on the
// two-CPU-group decline) lives in `crates/ynz-codegen/tests/golden.rs`; here we prove the
// HINT side fires exactly with the shared admission decision, so the two together pin both ends.

use std::collections::HashSet;

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::cpu_admission::{admitted_cpu_group, SuspendSet};
use ynz_typeck::parallel_group_hints;
use ynz_typeck::queries::{check_query, module_signatures_query};
use ynz_typeck::signatures::build_effective_suspend_set;

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_pgh_parity_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

/// The set of source lines (1-based) at which a CPU "separate core" hint fires.
fn cpu_hint_lines(db: &CompilerDb, sf: SourceFile) -> HashSet<usize> {
    let text = sf.text(db);
    parallel_group_hints(db, sf)
        .into_iter()
        .filter(|h| h.label.contains("separate core"))
        .map(|h| text[..h.position.min(text.len())].matches('\n').count() + 1)
        .collect()
}

/// The set of source lines (1-based) at which an I/O-overlap hint (no separate core) fires.
fn io_hint_lines(db: &CompilerDb, sf: SourceFile) -> HashSet<usize> {
    let text = sf.text(db);
    parallel_group_hints(db, sf)
        .into_iter()
        .filter(|h| !h.label.contains("separate core"))
        .map(|h| text[..h.position.min(text.len())].matches('\n').count() + 1)
        .collect()
}

/// Rebuild the inputs the admission gate needs (effective suspend set + supported callees) the
/// same way the inlay pass does, so this test reads the exact decision the pass reads.
fn admission_inputs(db: &CompilerDb, sf: SourceFile) -> (SuspendSet, HashSet<String>) {
    let sig_output = module_signatures_query(db, sf);
    let check_out = check_query(db, sf);
    let effective_suspends =
        build_effective_suspend_set(&check_out.suspends_set, &sig_output.imported_fns);
    let supported: HashSet<String> = sig_output
        .sig_table
        .fns
        .iter()
        .filter(|(_, sig)| ynz_typeck::independence::cpu_result_abi_supports(&sig.ret))
        .map(|(name, _)| name.clone())
        .collect();
    (effective_suspends, supported)
}

const FIB: &str = "\
function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
";

#[test]
fn clean_cpu_group_hint_fires_and_matches_admission() {
    // WHY: a single clean 2-member CPU group is the admitted case — codegen spawns it. The hint
    // must fire on BOTH members with "separate core", and `admitted_cpu_group` must return Some.
    let src = format!(
        "{FIB}\nfunction entrypoint() -> nothing {{\n  let a = fib(20)\n  let b = fib(21)\n  print(a)\n  print(b)\n}}\n"
    );
    let (db, sf) = single_file(&src);

    let lines = cpu_hint_lines(&db, sf);
    // fib body is 6 lines; the two member lines are 9 and 10 (`let a`, `let b`).
    assert_eq!(
        lines,
        HashSet::from([9, 10]),
        "clean CPU group must fire a separate-core hint on each member; got {lines:?}"
    );

    // The hint set must equal the admission decision.
    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported) = admission_inputs(&db, sf);
    let entry = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "entrypoint" => Some(f),
            _ => None,
        })
        .expect("entrypoint present");
    assert!(
        admitted_cpu_group(entry, &suspends, &supported).is_some(),
        "admission gate must admit the clean group (binary spawns it)"
    );
}

#[test]
fn two_cpu_groups_decline_emits_no_hint() {
    // WHY: a function with TWO CPU groups (one top-level pair + one nested-arm pair) is DECLINED
    // by the single-group constraint — codegen emits ZERO spawns. The hint must fire for NEITHER
    // group, or the IDE would claim both run on separate cores when the binary runs them
    // sequentially. This is the load-bearing hint↔binary agreement case.
    let src = format!(
        "{FIB}\nfunction entrypoint() -> nothing {{\n  let a = fib(10)\n  let b = fib(11)\n  print(a)\n  print(b)\n  if (a > 0) {{\n    let c = fib(12)\n    let d = fib(13)\n    print(c)\n    print(d)\n  }}\n}}\n"
    );
    let (db, sf) = single_file(&src);

    let lines = cpu_hint_lines(&db, sf);
    assert!(
        lines.is_empty(),
        "two-CPU-group function must emit NO separate-core hint (codegen spawns nothing); got {lines:?}"
    );

    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported) = admission_inputs(&db, sf);
    let entry = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "entrypoint" => Some(f),
            _ => None,
        })
        .expect("entrypoint present");
    assert!(
        admitted_cpu_group(entry, &suspends, &supported).is_none(),
        "admission gate must DECLINE a two-group function (no spawn → no hint)"
    );
}

#[test]
fn param_read_after_join_declines_emits_no_hint() {
    // WHY: a host whose parameter is READ after the join is declined (the spawn overwrites the
    // param slot). Codegen emits no spawn, so the hint must not fire.
    let src = format!(
        "{FIB}\nfunction host(p: int) -> int {{\n  let a = fib(10)\n  let b = fib(11)\n  return a + b + p\n}}\nfunction entrypoint() -> nothing {{\n  print(host(1))\n}}\n"
    );
    let (db, sf) = single_file(&src);

    let lines = cpu_hint_lines(&db, sf);
    assert!(
        lines.is_empty(),
        "param-read-after-join must emit NO separate-core hint; got {lines:?}"
    );

    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported) = admission_inputs(&db, sf);
    let host = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "host" => Some(f),
            _ => None,
        })
        .expect("host present");
    assert!(
        admitted_cpu_group(host, &suspends, &supported).is_none(),
        "admission gate must DECLINE a post-join param read"
    );
}

#[test]
fn suspending_fn_emits_io_hint_without_separate_core() {
    // WHY: a suspending (non-promoted) function never spawns onto a separate core. Two independent
    // suspending calls overlap cooperatively, so the hint must fire WITHOUT "separate core". This
    // proves the suspending-not-promoted path uses the I/O partition, not the CPU partition.
    let src = "\
function ioA() -> int {
  wait sleep(10)
  return 1
}
function ioB() -> int {
  wait sleep(10)
  return 2
}
function run() -> nothing {
  let a = ioA()
  let b = ioB()
  print(a)
  print(b)
}
";
    let (db, sf) = single_file(src);

    let cpu = cpu_hint_lines(&db, sf);
    assert!(
        cpu.is_empty(),
        "a suspending function must emit NO separate-core hint; got {cpu:?}"
    );
    let io = io_hint_lines(&db, sf);
    assert!(
        !io.is_empty(),
        "two independent suspending calls must emit an I/O-overlap hint"
    );
}

#[test]
fn cpu_group_with_trailing_wait_is_suspending_no_separate_core() {
    // WHY: the mixed case (CPU pair then `wait sleep`) makes the function SUSPENDING, so it routes
    // through the I/O path, not the CPU spawn path. The admission gate declines the CPU group
    // (the function has another suspension point), so NO "separate core" hint must appear — it
    // would lie about a spawn the binary's nested-group gate refuses.
    let src = format!(
        "{FIB}\nfunction run() -> nothing {{\n  let a = fib(10)\n  let b = fib(11)\n  wait sleep(0)\n  print(a)\n  print(b)\n}}\n"
    );
    let (db, sf) = single_file(&src);

    let cpu = cpu_hint_lines(&db, sf);
    assert!(
        cpu.is_empty(),
        "CPU pair + trailing wait must emit NO separate-core hint; got {cpu:?}"
    );
}
