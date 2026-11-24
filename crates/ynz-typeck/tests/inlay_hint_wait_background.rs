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

macro_rules! wait_point_cases {
    ($($case:ident: $body:block)+) => {
        $(
            // bug: `wait_points_hints` misses a call to a suspending function (local or imported, also buried in an operand, a struct field or an argument), or hints a call that is non-suspending or already `wait`ed | tier: Medium
            #[test]
            fn $case() $body
        )+
    };
}

wait_point_cases! {
    // `run()` calls `doWork()` without an explicit `wait`; `doWork` calls `sleep`, so it is in the
    // suspends set and the pass must hint it.
    wait_points_fire_for_call_to_suspending_fn: {
        let src = "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
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

    // `helper()` calls only `print`, which is not suspending.
    wait_points_suppressed_for_non_suspending_fn: {
        let src = "\
function helper() -> Nothing {
  print(`hello`)
}
function run() -> Nothing {
  helper()
}
";
        let (db, sf) = single_file(src);
        let hints = wait_points_hints(&db, sf);
        let for_helper: Vec<_> = hints.iter().filter(|h| h.callee_name == "helper").collect();
        assert!(
            for_helper.is_empty(),
            "non-suspending `helper` must not emit wait_points hint; got: {for_helper:?}"
        );
    }

    // The user already wrote `wait`, so a hint would be redundant.
    wait_points_suppressed_when_explicit_wait_present: {
        let src = "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
  wait doWork()
}
";
        let (db, sf) = single_file(src);
        let hints = wait_points_hints(&db, sf);
        let for_dowork: Vec<_> = hints.iter().filter(|h| h.callee_name == "doWork").collect();
        assert!(
            for_dowork.is_empty(),
            "explicit `wait doWork()` must suppress the wait_points hint; got: {for_dowork:?}"
        );
    }

    // Before the exhaustive-match fix `Expr::BinOp` fell into `_ => {}` and dropped the call.
    wait_points_fire_for_call_buried_in_binop: {
        let src = "\
function doWork() -> Int {
  wait sleep(100)
  return 1
}
function run() -> Nothing {
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

    // Before the exhaustive-match fix `Expr::StructLit` fell into `_ => {}` too.
    wait_points_fire_for_call_buried_in_struct_lit_field: {
        let src = "\
shape Wrapper { result: Int }
function doWork() -> Int {
  wait sleep(100)
  return 1
}
function run() -> Nothing {
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

    // `wait doWork(otherWork())`: `doWork` is explicitly waited (no hint) but `otherWork` is a
    // suspending call passed as an argument and keeps its own hint; the suppression is per call
    // site, not recursive into the arguments.
    wait_points_suppression_does_not_hide_a_suspending_argument: {
        let src = "\
function doWork() -> Nothing {
  wait sleep(100)
}
function otherWork() -> Nothing {
  wait sleep(50)
}
function run() -> Nothing {
  wait doWork(otherWork())
}
";
        let (db, sf) = single_file(src);
        let hints = wait_points_hints(&db, sf);
        let for_dowork: Vec<_> = hints.iter().filter(|h| h.callee_name == "doWork").collect();
        assert!(
            for_dowork.is_empty(),
            "explicit `wait doWork(...)` must suppress the hint for doWork itself; got: {for_dowork:?}"
        );
        let for_other: Vec<_> = hints
            .iter()
            .filter(|h| h.callee_name == "otherWork")
            .collect();
        assert!(
            !for_other.is_empty(),
            "suspending arg `otherWork()` inside `wait doWork(...)` must still get its own hint; got: {:?}",
            hints.iter().map(|h| &h.callee_name).collect::<Vec<_>>()
        );
    }

    // Before the effective-set fix the pass read only the LOCAL suspends set, so no hint fired at a
    // call to an imported suspending function while the binary treated it as a suspension point.
    wait_points_fire_for_imported_suspending_callee: {
        let dep_src = "\
export function slow() -> Nothing {
  wait sleep(200)
}
";
        let main_src = "\
import { slow } from `services/ops`
function run() -> Nothing {
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
}

macro_rules! background_routing_cases {
    ($($case:ident: $body:block)+) => {
        $(
            // bug: `background_routing_hints` labels a spawn with the wrong pool (I/O vs CPU, also for an imported suspending callee), or emits no or several hints per spawn site | tier: Medium
            #[test]
            fn $case() $body
        )+
    };
}

background_routing_cases! {
    // Mirrors the codegen routing predicate that sends suspending callees to `ynz_rt_spawn`.
    background_routing_io_pool_for_suspending_callee: {
        let src = "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
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

    // Mirrors the codegen path that sends non-suspending callees to `ynz_rt_spawn_blocking`; one
    // `background` expression is one routing hint (two would double the editor annotation).
    background_routing_cpu_pool_for_non_suspending_callee_once: {
        let src = "\
function task() -> Nothing {
  print(`task`)
}
function run() -> Nothing {
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
        assert!(
            hints[0].label.contains("CPU pool"),
            "non-suspending callee must emit CPU-pool routing hint; got: {:?}",
            hints[0].label
        );
    }

    background_routing_no_hints_without_a_background_spawn: {
        let src = "\
function run() -> Nothing {
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

    // Before the effective-set fix the hint read only the LOCAL suspends set and showed "CPU pool"
    // for `background importedSuspendingFn()` while codegen routed it to the I/O pool: the hint
    // was the exact opposite of the binary.
    background_routing_io_pool_for_imported_suspending_callee: {
        let dep_src = "\
export function slow() -> Nothing {
  wait sleep(200)
}
";
        let main_src = "\
import { slow } from `services/ops`
function run() -> Nothing {
  background slow()
}
";
        let (db, main_sf, _dir) = two_files_on_disk("ops", dep_src, main_src);
        let hints = background_routing_hints(&db, main_sf);
        assert!(
            hints.iter().any(|h| h.label.contains("I/O pool")),
            "imported suspending `slow` via background must emit I/O-pool hint (not CPU); got: {:?}",
            hints.iter().map(|h| &h.label).collect::<Vec<_>>()
        );
        assert!(
            !hints.iter().any(|h| h.label.contains("CPU pool")),
            "must NOT emit CPU-pool hint for imported suspending `slow`; got: {:?}",
            hints.iter().map(|h| &h.label).collect::<Vec<_>>()
        );
    }
}
