// `frame_layouts_query` sizes the frames the runtime allocates; `codegen_query` lays the same
// frames out and emits the code that writes them. Every case here is one way those two agree or
// the sizing is wrong (a wrong size is an out-of-bounds frame write at run time).

use std::path::PathBuf;

use ynz_abi::FRAME_HEADER_SIZE;
use ynz_codegen::state_machine::{own_locals_size, FRAME_SHAPE_REGION_SLACK_SLOTS};
use ynz_codegen::{codegen_query, frame_layouts_query};
use ynz_parser::{CompilerDb, SourceFile};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ynz-driver/tests/fixtures")
}

/// Register a fixture file in `db` using its canonical on-disk path.
///
/// The canonical path is required because `resolve_module_path` calls `fs::canonicalize`
/// when resolving cross-file imports — both sides must use the same canonical form so
/// `db.source_by_path(canonical_path)` finds the entry.
fn register_fixture(db: &mut CompilerDb, fixture_dir: &str, filename: &str) -> SourceFile {
    let path = fixtures_dir().join(fixture_dir).join(filename);
    let canonical = std::fs::canonicalize(&path)
        .unwrap_or_else(|error| panic!("cannot canonicalize {}: {error}", path.display()));
    let text = std::fs::read_to_string(&canonical)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", canonical.display()));
    let canonical_str = canonical.display().to_string();
    let source_file = SourceFile::new(db, canonical_str, text);
    db.register_source(source_file);
    source_file
}

/// Register an inline source string in `db` under `path`.
fn register_inline(db: &mut CompilerDb, path: &str, source: &str) -> SourceFile {
    let source_file = SourceFile::new(db, path.to_string(), source.to_string());
    db.register_source(source_file);
    source_file
}

/// One named test per case so the runner names the failing case. `register` puts the case's
/// files in the database and returns the one to query; `function` must have exactly the stated
/// own slots and total frame size.
macro_rules! frame_layout_case {
    ($test_name:ident, register: $register:expr, function: $function:expr, n_locals: $n_locals:expr, total_size: $total_size:expr) => {
        // bug: a function's frame is sized differently from what the runtime and the emitted code use (a crossing local, an imported callee's sub-frame or a shape is counted wrong), so a frame write goes out of bounds | tier: High
        #[test]
        fn $test_name() {
            let mut db = CompilerDb::default();
            let register: fn(&mut CompilerDb) -> SourceFile = $register;
            let source_file = register(&mut db);
            let layouts = frame_layouts_query(&db, source_file);
            let layout = layouts.get($function).unwrap_or_else(|| {
                panic!(
                    "no layout for `{}`; got keys: {:?}",
                    $function,
                    layouts.keys().collect::<Vec<_>>()
                )
            });
            assert_eq!(layout.n_locals, $n_locals);
            assert_eq!(layout.total_size, $total_size);
        }
    };
}

// Case bug: a crossing local live across a suspension is not counted (frame stays header-only, 32).
frame_layout_case!(
    a_crossing_int_local_adds_one_slot,
    register: |db: &mut CompilerDb| register_fixture(db, "v0_3_m3e_reexport_chain_int", "a_ops.ynz"),
    function: "getValue",
    n_locals: 1,
    total_size: FRAME_HEADER_SIZE + own_locals_size(1)
);

// Case bug: an imported suspending callee's sub-frame is sized as a bare header (32) instead of
// its real size (40), so the caller's frame is too small for the embedded child.
frame_layout_case!(
    an_imported_suspending_callee_embeds_its_real_sub_frame,
    register: |db: &mut CompilerDb| {
        register_fixture(db, "v0_3_m3e_reexport_chain_int", "a_ops.ynz");
        register_fixture(db, "v0_3_m3e_reexport_chain_int", "b_ops.ynz")
    },
    function: "doWork",
    n_locals: 0,
    total_size: FRAME_HEADER_SIZE + own_locals_size(0) + FRAME_HEADER_SIZE + own_locals_size(1)
);

// Case bug: a shape wider than one word (three Ints, 24 bytes ABI) is sized as one slot when the
// LLVM sizing silently fails; the frame also carries the moved-flag slot and the region slack.
frame_layout_case!(
    a_three_int_shape_crossing_local_takes_three_slots,
    register: |db: &mut CompilerDb| register_inline(
        db,
        "test_three_int_shape.ynz",
        r#"
shape Point3D {
  x: Int
  y: Int
  z: Int
}

function caller() -> Nothing {
  let pos: Point3D = { x: 1, y: 2, z: 3 }
  sleep(10)
  print(pos.x.toString())
}

function entrypoint() -> Nothing {
  caller()
}
"#
    ),
    function: "caller",
    n_locals: 1 + 3 + FRAME_SHAPE_REGION_SLACK_SLOTS,
    total_size: FRAME_HEADER_SIZE + own_locals_size(1 + 3 + FRAME_SHAPE_REGION_SLACK_SLOTS)
);

// Case bug: a compact shape (two Booleans, 2 bytes ABI) is sized by field count (two slots)
// instead of the LLVM ABI size (one slot), shifting every later slot.
frame_layout_case!(
    a_two_boolean_shape_crossing_local_takes_one_slot,
    register: |db: &mut CompilerDb| register_inline(
        db,
        "test_bool_shape.ynz",
        r#"
shape Flags {
  flag: Boolean
  flag2: Boolean
}

function caller() -> Nothing {
  let flags: Flags = { flag: true, flag2: false }
  sleep(10)
  print(flags.flag.toString())
}

function entrypoint() -> Nothing {
  caller()
}
"#
    ),
    function: "caller",
    n_locals: 1 + 1 + FRAME_SHAPE_REGION_SLACK_SLOTS,
    total_size: FRAME_HEADER_SIZE + own_locals_size(1 + 1 + FRAME_SHAPE_REGION_SLACK_SLOTS)
);

const IO_WORK_POST_PAIR_SRC: &str = r#"
function fib(n: Int) -> Int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
function ioWork() -> Nothing {
    wait sleep(0)
}
function entrypoint() -> Nothing {
    let a = fib(10)
    let b = fib(11)
    ioWork()
    print(a)
    print(b)
}
"#;

/// One named test per case so the runner names the failing case. A CPU pair followed by a call to
/// a suspending function is declined as a spike host: `codegen_query` must emit no joinable spawn
/// AND `frame_layouts_query` must have sized `entrypoint` sequentially, with the suspending callee
/// as a child sub-frame. The two queries answering differently under-allocates the heap block by
/// exactly that sub-frame.
macro_rules! declined_spike_host_case {
    ($test_name:ident, register: $register:expr) => {
        // bug: frame_layouts_query and codegen_query disagree on whether a function is a spike host (one sizes its frame sequentially with the suspending callee's sub-frame, the other lays it out as a host without it), so the heap block is under-allocated and the child-frame write corrupts memory | tier: High
        #[test]
        fn $test_name() {
            let mut db = CompilerDb::default();
            let register: fn(&mut CompilerDb) -> SourceFile = $register;
            let source_file = register(&mut db);

            let codegen_output = codegen_query(&db, source_file);
            assert!(
                !codegen_output.diagnostics.has_errors(),
                "{:?}",
                codegen_output.diagnostics
            );
            let spawn_calls = codegen_output
                .artifact
                .ir_text
                .lines()
                .filter(|line| {
                    line.contains("call") && line.contains("@ynz_rt_spawn_blocking_joinable")
                })
                .count();
            assert_eq!(spawn_calls, 0, "a declined spike host must spawn nothing");

            let layouts = frame_layouts_query(&db, source_file);
            let entrypoint = layouts
                .get("entrypoint")
                .expect("entrypoint has a frame layout");
            assert!(
                entrypoint
                    .children
                    .iter()
                    .any(|(callee, _)| callee == "ioWork"),
                "entrypoint is sized sequentially, with ioWork embedded; got {:?}",
                entrypoint.children
            );
        }
    };
}

// Case bug: the suspending callee is imported, so a bare local-only suspend set misses it.
declined_spike_host_case!(
    an_imported_suspending_call_after_a_cpu_pair_is_declined_by_both_queries,
    register: |db: &mut CompilerDb| {
        let fixture_dir = "v0_3_m3d_spike_s_imported_suspending_after_pair";
        register_fixture(db, fixture_dir, "io_lib.ynz");
        register_fixture(db, fixture_dir, "entrypoint.ynz")
    }
);
// Case bug: the suspending callee is local, so the post-pair gate is the only thing declining.
declined_spike_host_case!(
    a_local_suspending_call_after_a_cpu_pair_is_declined_by_both_queries,
    register: |db: &mut CompilerDb| register_inline(db, "entrypoint.ynz", IO_WORK_POST_PAIR_SRC)
);

/// One named test per case so the runner names the failing case.
macro_rules! empty_layout_case {
    ($test_name:ident, path: $path:expr, source: $source:expr) => {
        // bug: a module that has nothing to size (no suspending function, or a checker error) gets phantom or partial frame layouts, or crashes the query | tier: Medium
        #[test]
        fn $test_name() {
            let mut db = CompilerDb::default();
            let source_file = register_inline(&mut db, $path, $source);
            let layouts = frame_layouts_query(&db, source_file);
            assert!(
                layouts.is_empty(),
                "got: {:?}",
                layouts.keys().collect::<Vec<_>>()
            );
        }
    };
}

// Case bug: a module with no suspending function (the common case) gets layout entries.
empty_layout_case!(
    a_module_with_no_suspending_function_has_no_layouts,
    path: "no_suspends.ynz",
    source: "function entrypoint() -> Nothing {\n  print(`hello`)\n}\n"
);
// Case bug: a module whose import does not resolve (no such file) gets layouts from a module
// the checker rejected.
empty_layout_case!(
    a_module_with_an_unresolvable_import_has_no_layouts,
    path: "/tmp/ynz_test_cycle/a_cycle.ynz",
    source: "import { doB } from `b_cycle`\n\nexport function doA() -> Nothing {\n  sleep(10)\n}\n"
);
