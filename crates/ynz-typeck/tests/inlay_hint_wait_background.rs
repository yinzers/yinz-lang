// WHY: unit tests for the v0.3-M3b `wait_points_hints` and
// `background_routing_hints` salsa-tracked inlay-hint passes.
//
// `wait_points_hints` emits a hint before each call to a function in
// the EFFECTIVE suspend set (local + imported suspending names) that is NOT
// already wrapped in explicit `wait`.
//
// `background_routing_hints` emits a routing comment at each `background`
// spawn site, reading the same effective suspend set that drives the codegen
// routing predicate at `crates/ynz-codegen/src/emit.rs:9335`.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{background_routing_hints, wait_points_hints};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_ih_wb_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

/// Register two on-disk files (dep at `services/<name>.ynz`, main at `entrypoint.ynz`)
/// and return the main `SourceFile`.  The `_dir` return value is a `TempDir`-style
/// path whose lifetime keeps the temp files alive for the test.
fn two_files_on_disk(
    services_name: &str,
    services_src: &str,
    main_src: &str,
) -> (CompilerDb, SourceFile, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "ynz_ih_wb_cross_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(dir.join("services")).expect("create dirs");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").unwrap();

    let dep_path = dir.join("services").join(format!("{services_name}.ynz"));
    std::fs::write(&dep_path, services_src).expect("write dep");

    let main_path = dir.join("entrypoint.ynz");
    std::fs::write(&main_path, main_src).expect("write main");

    let dep_path_str = dep_path.canonicalize().unwrap().display().to_string();
    let main_path_str = main_path.canonicalize().unwrap().display().to_string();

    let mut db = CompilerDb::default();
    let dep_sf = SourceFile::new(&db, dep_path_str, services_src.to_string());
    db.register_source(dep_sf);
    let main_sf = SourceFile::new(&db, main_path_str, main_src.to_string());
    db.register_source(main_sf);

    (db, main_sf, dir)
}

// ─────────────────────────────────────────────────────────────────────────────
// wait_points_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_wait_points_fires_for_call_to_suspending_fn() {
    // WHY: `run()` calls `doWork()` without explicit `wait`. `doWork` calls
    // `sleep` and is therefore in `suspends_set`. The pass must emit a
    // WaitPointHint at `doWork()`'s byte position inside `run`.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  doWork()
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    assert!(
        hints.iter().any(|h| h.callee_name == "doWork"),
        "wait_points_hints must fire for call to suspending `doWork`; got: {:?}",
        hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
    );
}

#[test]
fn test_wait_points_suppressed_for_non_suspending_fn() {
    // WHY: `helper()` calls only `print`, which is NOT in `suspends_set`.
    // No WaitPointHint should fire for `helper()`.
    let src = "\
function helper() -> nothing {
  print(`hello`)
}
function run() -> nothing {
  helper()
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    let for_helper: Vec<_> = hints
        .iter()
        .filter(|h| h.callee_name == "helper")
        .collect();
    assert!(
        for_helper.is_empty(),
        "non-suspending `helper` must not emit wait_points hint; got: {:?}",
        for_helper
    );
}

#[test]
fn test_wait_points_suppressed_when_explicit_wait_present() {
    // WHY: `wait doWork()` — the user already wrote `wait`, so the pass must
    // NOT emit a redundant hint for this call site.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  wait doWork()
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    let for_dowork: Vec<_> = hints
        .iter()
        .filter(|h| h.callee_name == "doWork")
        .collect();
    assert!(
        for_dowork.is_empty(),
        "explicit `wait doWork()` must suppress the wait_points hint; got: {:?}",
        for_dowork
    );
}

#[test]
fn test_wait_points_position_is_before_call_start() {
    // WHY: Addition-placement hints render BEFORE the call — the hint byte
    // position must be <= the first byte of the call expression.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  doWork()
}
";
    let call_offset = src.rfind("doWork()").expect("doWork() not in source");
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    let matching: Vec<_> = hints
        .iter()
        .filter(|h| h.callee_name == "doWork")
        .collect();
    assert!(
        !matching.is_empty(),
        "expected at least one wait_points hint for doWork()"
    );
    for h in &matching {
        assert!(
            h.position <= call_offset + "doWork".len(),
            "hint position {} must be at or before call end {} (Addition placement = before the call)",
            h.position,
            call_offset
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// background_routing_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_background_routing_io_pool_for_suspending_callee() {
    // WHY: `background doWork()` where `doWork` is in `suspends_set` must emit
    // a routing hint labelled "I/O pool". This mirrors the codegen routing
    // predicate at emit.rs:9335 that routes suspending callees to `ynz_rt_spawn`.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  background doWork()
}
";
    let (db, sf) = single_file(src);
    let hints = background_routing_hints(&db, sf);
    let io_hint = hints
        .iter()
        .find(|h| h.label.contains("I/O pool") && h.label.contains("doWork"));
    assert!(
        io_hint.is_some(),
        "suspending callee `doWork` must emit I/O-pool routing hint; got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_background_routing_cpu_pool_for_non_suspending_callee() {
    // WHY: `background helper()` where `helper` does NOT call any suspending
    // intrinsic must emit a routing hint labelled "CPU pool". Mirrors the
    // codegen path that routes non-suspending callees to `ynz_rt_spawn_blocking`.
    let src = "\
function helper() -> nothing {
  print(`working`)
}
function run() -> nothing {
  background helper()
}
";
    let (db, sf) = single_file(src);
    let hints = background_routing_hints(&db, sf);
    let cpu_hint = hints.iter().find(|h| h.label.contains("CPU pool"));
    assert!(
        cpu_hint.is_some(),
        "non-suspending callee must emit CPU-pool routing hint; got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_background_routing_fires_exactly_once_per_spawn_site() {
    // WHY: one `background` expression = one routing comment. Double-emitting
    // would produce duplicate annotations in the editor.
    let src = "\
function task() -> nothing {
  print(`task`)
}
function run() -> nothing {
  background task()
}
";
    let (db, sf) = single_file(src);
    let hints = background_routing_hints(&db, sf);
    assert_eq!(
        hints.len(),
        1,
        "one background spawn must produce exactly one routing hint; got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_background_routing_no_hints_for_plain_fn_with_no_background() {
    // WHY: a function with no `background` spawn emits no routing hints.
    let src = "\
function run() -> nothing {
  print(`no background here`)
}
";
    let (db, sf) = single_file(src);
    let hints = background_routing_hints(&db, sf);
    assert!(
        hints.is_empty(),
        "no background spawn = no routing hints; got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Compound-expression recursion regression tests (FIX 1 / FIX 2)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_wait_points_fires_for_call_buried_in_binop() {
    // WHY: before the exhaustive-match fix, `collect_wait_point_hints_expr` fell
    // into `_ => {}` for `Expr::BinOp`, silently dropping any suspending call
    // buried in either operand.  `let x = doWork() + 1` must fire a hint for
    // `doWork` even though the call is the LHS of an addition.
    let src = "\
function doWork() -> int {
  wait sleep(100)
  return 1
}
function run() -> nothing {
  let x = doWork() + 1
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    assert!(
        hints.iter().any(|h| h.callee_name == "doWork"),
        "wait_points_hints must fire for `doWork()` buried in a BinOp; got: {:?}",
        hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
    );
}

#[test]
fn test_wait_points_fires_for_call_buried_in_struct_lit_field() {
    // WHY: before the exhaustive-match fix, `Expr::StructLit` fell into `_ => {}`
    // and any suspending call in a struct field was silently skipped.
    // `let v = { result: doWork() }` must fire a hint for `doWork`.
    let src = "\
shape Wrapper { result: int }
function doWork() -> int {
  wait sleep(100)
  return 1
}
function run() -> nothing {
  let v: Wrapper = { result: doWork() }
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);
    assert!(
        hints.iter().any(|h| h.callee_name == "doWork"),
        "wait_points_hints must fire for `doWork()` in a StructLit field; got: {:?}",
        hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
    );
}

#[test]
fn test_wait_points_suppressed_with_suspending_arg_still_fires_for_inner() {
    // WHY: `wait doWork(otherWork())` — `doWork` is explicitly waited so it must
    // NOT get a hint, but `otherWork` is a suspending call PASSED AS AN ARGUMENT
    // and must still get its own hint.  The suppression must be per-call-site,
    // not recursive into args (that would hide real teaching opportunities).
    //
    // This is a falsifiability test for the suppression logic: if the inner-arg
    // recursion arm were deleted from `collect_wait_point_hints_expr_no_top`, the
    // `otherWork` hint would disappear and this test would fail.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function otherWork() -> nothing {
  wait sleep(50)
}
function run() -> nothing {
  wait doWork(otherWork())
}
";
    let (db, sf) = single_file(src);
    let hints = wait_points_hints(&db, sf);

    let for_dowork: Vec<_> = hints.iter().filter(|h| h.callee_name == "doWork").collect();
    assert!(
        for_dowork.is_empty(),
        "explicit `wait doWork(...)` must suppress the hint for doWork itself; got: {:?}",
        for_dowork
    );

    let for_other: Vec<_> = hints.iter().filter(|h| h.callee_name == "otherWork").collect();
    assert!(
        !for_other.is_empty(),
        "suspending arg `otherWork()` inside `wait doWork(...)` must still get its own hint; got: {:?}",
        hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-module effective-suspend-set tests (FIX 3)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_wait_points_fires_for_imported_suspending_callee() {
    // WHY: before the effective-set fix, `wait_points_hints` read bare
    // `check.suspends_set` which only contains LOCAL function names.  An imported
    // suspending function (from another module) was missing from that set, so no
    // hint fired at its call site even though the binary correctly treats it as
    // a suspension point.  The hint and the binary diverged for every cross-module
    // suspending call.
    let dep_src = "\
export function slow() -> nothing {
  wait sleep(200)
}
";
    let main_src = "\
import { slow } from `services/ops`
function run() -> nothing {
  slow()
}
";
    let (db, main_sf, _dir) = two_files_on_disk("ops", dep_src, main_src);
    let hints = wait_points_hints(&db, main_sf);
    assert!(
        hints.iter().any(|h| h.callee_name == "slow"),
        "wait_points_hints must fire for imported suspending `slow`; got: {:?}",
        hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
    );
}

#[test]
fn test_background_routing_io_pool_for_imported_suspending_callee() {
    // WHY: before the effective-set fix, `background_routing_hints` read bare
    // `check.suspends_set` (local-only), so `background importedSuspendingFn()`
    // showed "CPU pool" in the hint while codegen correctly routes it to the I/O
    // pool — the hint was the EXACT OPPOSITE of reality (confirmed live).
    // The effective set (local + imported-suspending from module_signatures_query)
    // must be used so hint and binary always agree.
    let dep_src = "\
export function slow() -> nothing {
  wait sleep(200)
}
";
    let main_src = "\
import { slow } from `services/ops`
function run() -> nothing {
  background slow()
}
";
    let (db, main_sf, _dir) = two_files_on_disk("ops", dep_src, main_src);
    let hints = background_routing_hints(&db, main_sf);
    let io_hints: Vec<_> = hints.iter().filter(|h| h.label.contains("I/O pool")).collect();
    assert!(
        !io_hints.is_empty(),
        "imported suspending `slow` via background must emit I/O-pool hint (not CPU); got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
    let cpu_hints: Vec<_> = hints.iter().filter(|h| h.label.contains("CPU pool")).collect();
    assert!(
        cpu_hints.is_empty(),
        "must NOT emit CPU-pool hint for imported suspending `slow`; got: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}
