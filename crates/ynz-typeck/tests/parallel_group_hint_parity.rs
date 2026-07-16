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
use ynz_typeck::cpu_admission::{admitted_cpu_group, admitted_fused_group, SuspendSet};
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
fn admission_inputs(
    db: &CompilerDb,
    sf: SourceFile,
) -> (
    SuspendSet,
    HashSet<String>,
    ynz_typeck::cpu_admission::ExprTypes,
) {
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
    let expr_types = check_out.typed_module.expr_types.clone();
    (effective_suspends, supported, expr_types)
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
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
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
        admitted_cpu_group(entry, &suspends, &supported, &expr_types).is_some(),
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
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
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
        admitted_cpu_group(entry, &suspends, &supported, &expr_types).is_none(),
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
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
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
        admitted_cpu_group(host, &suspends, &supported, &expr_types).is_none(),
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

// ── v0.3-M3g Phase 5: mixed (fused) CPU+I/O group parity ─────────────────────────────────────

const CRUNCH_FETCH: &str = "\
function crunch(n: int) -> int {
  let total = 0
  let i = 0
  while (i < 30) {
    total = total + i
    i = i + 1
  }
  return total + n
}

function fetchA(n: int) -> int {
  wait sleep(0)
  return n + 10
}
";

/// 1-based line number of the first source line containing `needle`.
fn line_of(src: &str, needle: &str) -> usize {
    src.lines()
        .position(|l| l.contains(needle))
        .map(|i| i + 1)
        .unwrap_or_else(|| panic!("no line containing {needle:?} in source"))
}

#[test]
fn mixed_fused_group_hint_tags_cpu_and_io_members_and_matches_admission() {
    // WHY: before this fix, `admitted_fused_group` was invisible to the hint pass entirely (it
    // only ever read `admitted_cpu_group`), so a function whose CPU+I/O group genuinely fuses at
    // codegen (`emit_fused_group_spawn_poll`) rendered NO hint at all — worse than a stale hint,
    // since the teaching surface went silent exactly where the milestone's new behavior fires.
    // This proves the hint now fires on BOTH members, tags the CPU member "separate core" and the
    // Suspending member WITHOUT it, and that the SAME `admitted_fused_group` call the hint reads
    // is the one codegen's `fused_admitted_group` reads too — the E5 hint==binary parity proof,
    // extended over a genuinely mixed shape (the two existing parity fixtures above only ever
    // cover pure-CPU or pure-I/O groups).
    let src = format!(
        "{CRUNCH_FETCH}\nfunction entrypoint() -> nothing {{\n  let a = crunch(1)\n  let b = fetchA(2)\n  print(a + b)\n}}\n"
    );
    let (db, sf) = single_file(&src);

    let cpu_line = line_of(&src, "let a = crunch(1)");
    let io_line = line_of(&src, "let b = fetchA(2)");

    let cpu = cpu_hint_lines(&db, sf);
    assert_eq!(
        cpu,
        HashSet::from([cpu_line]),
        "the CPU member of a mixed fused group must get a separate-core hint on its own line; got {cpu:?}"
    );
    let io = io_hint_lines(&db, sf);
    assert_eq!(
        io,
        HashSet::from([io_line]),
        "the Suspending member of a mixed fused group must get an overlap hint WITHOUT separate core; got {io:?}"
    );

    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
    let entry = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "entrypoint" => Some(f),
            _ => None,
        })
        .expect("entrypoint present");
    let fused = admitted_fused_group(entry, &suspends, &supported, &expr_types)
        .expect("admission gate must admit the mixed group (binary fuses it)");
    assert_eq!(
        fused.members.len(),
        2,
        "the admitted fused group must have exactly 2 members"
    );
}

#[test]
fn mixed_fused_group_declines_on_non_scalar_arg_emits_no_hint() {
    // WHY: a call with 2 arguments is not a fused-group-eligible member (`admitted_fused_group`
    // requires exactly one `IntLit`/`Ident` argument per member, mirroring the pure-CPU path's
    // existing restriction). The group must NOT admit — and, per the same reasoning the pure-CPU
    // decline parity tests above already lock, NO hint of any kind should fire: codegen falls
    // back to fully sequential lowering for a declined group, so any hint here would lie about a
    // spawn/poll that never happens.
    let src = "\
function crunch2(n: int, m: int) -> int {
  return n + m
}
function fetchA(n: int) -> int {
  wait sleep(0)
  return n + 10
}
function entrypoint() -> nothing {
  let a = crunch2(1, 2)
  let b = fetchA(3)
  print(a + b)
}
";
    let (db, sf) = single_file(src);

    let cpu = cpu_hint_lines(&db, sf);
    assert!(
        cpu.is_empty(),
        "a non-scalar-arg member must never fire a separate-core hint; got {cpu:?}"
    );
    let io = io_hint_lines(&db, sf);
    assert!(
        io.is_empty(),
        "a declined fused group must emit NO overlap hint at all (codegen spawns/polls nothing); got {io:?}"
    );

    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
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
        admitted_fused_group(entry, &suspends, &supported, &expr_types).is_none(),
        "admission gate must decline a member with a non-scalar argument"
    );
}

#[test]
fn nested_suspending_group_in_ordinary_host_emits_hint_exactly_once_per_member() {
    // WHY: blocker fix (code-reviewer). The Phase 5 refactor split `collect_io_overlap_hints` into
    // a top-level pass that STILL internally recurses nested blocks (it calls
    // `collect_io_overlap_hints_in_nested_blocks` itself), while the suspending-function branch's
    // `else` (ordinary, non-fused) arm ALSO hoisted a second, unconditional call to that same
    // nested-blocks helper after the if/else. A suspending host with NO top-level fused group but
    // an independent-suspend group nested inside `if`/`while`/`for`/`match` got that nested
    // group's hints emitted TWICE — breaking the E5 "hint set == codegen spawn set" promise
    // (duplicate hints ≠ what codegen actually does; codegen inline-polls each member once). This
    // proves each member gets exactly ONE hint, not two.
    let src = "\
function ioA() -> int {
  wait sleep(10)
  return 1
}
function ioB() -> int {
  wait sleep(10)
  return 2
}
function run(flag: bool) -> nothing {
  if (flag) {
    let a = ioA()
    let b = ioB()
    print(a)
    print(b)
  }
}
";
    let (db, sf) = single_file(src);

    // No top-level fused (or CPU) group exists — both suspending calls are nested inside `if`, so
    // this exercises the ordinary `else` arm of the suspending-function branch, not the fused arm.
    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
    let entry = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "run" => Some(f),
            _ => None,
        })
        .expect("run present");
    assert!(
        admitted_fused_group(entry, &suspends, &supported, &expr_types).is_none(),
        "the two suspending calls are nested inside `if`, not top-level — no fused group should admit"
    );

    let a_line = line_of(src, "let a = ioA()");
    let b_line = line_of(src, "let b = ioB()");

    let hints = parallel_group_hints(&db, sf);
    let text = sf.text(&db);
    let count_at = |line: usize| {
        hints
            .iter()
            .filter(|h| text[..h.position.min(text.len())].matches('\n').count() + 1 == line)
            .count()
    };
    assert_eq!(
        count_at(a_line),
        1,
        "member `a` (nested independent-suspend group) must get exactly ONE hint, not a \
         double-emitted duplicate"
    );
    assert_eq!(
        count_at(b_line),
        1,
        "member `b` (nested independent-suspend group) must get exactly ONE hint, not a \
         double-emitted duplicate"
    );
}

#[test]
fn fused_member_that_is_itself_a_promoted_cpu_host_matches_codegen_real_suspend_set() {
    // WHY: should-fix (code-reviewer). Before this fix, the hint pass classified a fused group's
    // members against `effective_suspends` (the narrow, pre-CPU-promotion suspend set) while
    // codegen's real call site classifies against `suspends_with_promotions`
    // (`base_suspend_set ∪ spike_hosts`, `queries.rs`'s `codegen_query`/`frame_layouts_query`).
    // `pureHelper` is itself a promoted CPU-group host (its own `crunch(3)`/`crunch(4)` pair
    // admits) — under codegen's real set it is classified `Suspending` (it is in `spike_hosts`),
    // so `entrypoint`'s group (`pureHelper`, `ioWork`) has TWO `Suspending` members, is NOT
    // mixed, and `admitted_fused_group` correctly returns `None`. Under the pre-fix narrow set,
    // `pureHelper` was NOT in `effective_suspends`, so it classified `Cpu` instead — the group
    // looked mixed and admitted, and the hint would have falsely tagged `pureHelper`'s call
    // "separate core" for a group codegen never fuses. This test proves BOTH ends now agree:
    // `admitted_fused_group` under the real unioned set (constructed here the same way
    // `ynz_codegen::emit::spike_host_subset` does) returns `None`, and the actual hint pass's
    // output carries NO "separate core" tag anywhere in `entrypoint`.
    let src = "\
function crunch(seed: int) -> int {
  let total = 0
  let i = 0
  while (i < 50) {
    total = total + i
    i = i + 1
  }
  return total + seed
}

function pureHelper(seed: int) -> int {
  let a = crunch(3)
  let b = crunch(4)
  return a + b
}

function ioWork(n: int) -> int {
  wait sleep(0)
  return n
}

function entrypoint() -> nothing {
  let x = pureHelper(1)
  let y = ioWork(4)
  print(x + y)
}
";
    let (db, sf) = single_file(src);

    let parse = ynz_parser::parse_query(&db, sf);
    let (suspends, supported, expr_types) = admission_inputs(&db, sf);
    let pure_helper = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "pureHelper" => Some(f),
            _ => None,
        })
        .expect("pureHelper present");
    assert!(
        admitted_cpu_group(pure_helper, &suspends, &supported, &expr_types).is_some(),
        "sanity: pureHelper must be a genuine, self-admitting CPU-group host for this test to \
         actually exercise the spike_hosts union"
    );

    let entry = parse
        .module
        .items
        .iter()
        .find_map(|it| match it {
            ynz_ast::nodes::Item::Function(f) if f.name == "entrypoint" => Some(f),
            _ => None,
        })
        .expect("entrypoint present");

    // Codegen's real suspend set: `effective_suspends` unioned with every promoted candidate
    // whose own group `admitted_cpu_group` admits — the exact shape
    // `ynz_codegen::emit::spike_host_subset` computes (re-derived here rather than called
    // directly, since `ynz-typeck` cannot depend on `ynz-codegen`).
    let mut suspends_with_promotions = suspends.clone();
    suspends_with_promotions.insert("pureHelper".to_string());
    assert!(
        admitted_fused_group(entry, &suspends_with_promotions, &supported, &expr_types).is_none(),
        "codegen's real (unioned) suspend set must classify BOTH `pureHelper` and `ioWork` as \
         Suspending (pureHelper is a spike host) — not mixed, so the fused group must decline"
    );

    // The narrow, pre-fix set would have wrongly admitted a mixed group here — confirm that
    // divergence is real (documents WHY the fix is needed, doesn't re-test the fix itself).
    assert!(
        admitted_fused_group(entry, &suspends, &supported, &expr_types).is_some(),
        "sanity: the narrow effective-suspends set must (wrongly) classify pureHelper as Cpu, \
         admitting a mixed group — this is the exact divergence the fix closes"
    );

    // The actual hint pass output must match codegen's real decision on `entrypoint`'s call to
    // `pureHelper`: NO "separate core" tag there — codegen's real (unioned) suspend set declines
    // the fused group entirely, so `pureHelper` is never spawned from `entrypoint`. (`pureHelper`
    // legitimately DOES get its own "separate core" hints on ITS OWN `crunch(3)`/`crunch(4)`
    // lines — that is a separate, correct admission of pureHelper's OWN group and is untouched by
    // this fix; this assertion is scoped to entrypoint's call site, not the whole file.)
    let call_line = line_of(src, "let x = pureHelper(1)");
    let cpu = cpu_hint_lines(&db, sf);
    assert!(
        !cpu.contains(&call_line),
        "the hint pass must NOT tag entrypoint's `pureHelper(1)` call \"separate core\" — \
         codegen's real (unioned) suspend set declines the fused group entirely; got {cpu:?}"
    );
}
