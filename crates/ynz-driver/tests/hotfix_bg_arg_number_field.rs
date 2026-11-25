//! Hotfix `bgarg-number` (parked item 40 / M8 FRAGO 012): a shape with a `number`
//! (decimal128) field passed to a `background` spawn. Shipped bug on v0.3.3: the compiled
//! program died with signal 11 (SIGSEGV) in the default optimized build; the identical program
//! without the `number` field, or at `YNZ_NO_OPTIMIZE=1`, ran clean.
//!
//! Root cause (IR + asm evidence: `git log --grep=bgarg-number`): the caller keeps reading
//! `scene` after the spawn, so `scene` is a crossing local frame-EMBEDDED in the spawner's
//! state-machine frame at an 8-byte slot offset (`base + 40`). Its LLVM struct carries an i128
//! field, so the struct's ABI alignment is 16, and every access through the region pointer (the
//! spawn-site whole-struct copy and the caller's own `scene.scale` read) is emitted at
//! `align 16`. Optimized X86 ISel honors that claim with `movaps`, which faults on an address
//! that is 8 mod 16. The fix lives at the one producer of the region pointer
//! (`state_machine::shape_frame_region_ptr`): the region is rounded up to
//! `FRAME_SHAPE_REGION_ALIGN` and the layout reserves the slack for it.
//!
//! A `number` PARAMETER (not a field) crossing the same spawn boundary is asserted by
//! `v03_m6_number_spawn_boundary.rs`.

mod support;

use support::{assert_fixture_prints, build, run_counted};

/// The RED pin fixture (direct `number` field, channel spawn form, inferred copy).
const FIXTURE: &str = "bg_arg_shape_number_field.ynz";

/// One named test per spawn form of the `bg arg shape with a number field` unit; each case is a
/// test the runner names.
macro_rules! bg_arg_number_field_high_case {
    ($($case_name:ident, $fixture:expr, $expected_stdout:literal;)+) => { $(
        // bug: a shape with a `number` field passed to a `background` spawn is copied from a frame region that is 8 mod 16, so the `align 16` movaps faults (SIGSEGV) in the optimized build | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_prints(
                $fixture,
                false,
                $expected_stdout,
                "every field must reach the task and the caller intact",
            );
        }
    )+ };
}

bg_arg_number_field_high_case! {
    // case: the inferred-copy channel form: spawn-site whole-struct copy and the caller's post-resume `scene.scale` read both fault
    bg_arg_shape_with_number_field_survives_spawn, FIXTURE, "Point 800 600 1.5\n1 1.5\n";
    // case: the handle form (`let h = background f(scene)`) routes the same copy from the same misaligned region
    bg_arg_shape_with_number_field_handle_form_survives_spawn, "bg_arg_shape_number_field_handle.ynz", "task sees 1.5\nhandle saw 6 Three Rivers\n";
    // case: a `number` field one level down in a nested shape, give path (the copy-path twin is compile-gated)
    bg_arg_shape_with_number_field_nested_survives_spawn, "bg_arg_shape_number_field_nested.ynz", "task sees 2.5\ntasks saw 1\n";
}

// bug: the spawn-site shape heap copy of a `number`-field shape is freed other than exactly once by the task's drop ladder (leak or double free) | tier: High
#[test]
fn bg_arg_shape_with_number_field_alloc_free_parity() {
    let built = build(FIXTURE, false);
    let (run, counters) = run_counted(&built);
    assert_eq!(run.exit_code, 0, "must exit clean; stderr:\n{}", run.stderr);
    assert!(
        counters.alloc > 0,
        "the spawn-site shape heap copy must be counted (alloc=0 means the counter never \
         observed the copy); {counters:?}"
    );
    assert_eq!(
        counters.alloc, counters.free,
        "every spawn-site allocation must be freed exactly once; {counters:?}"
    );
}

// bug: the spawner's frame region for a `number`-field shape bypasses the aligning producer, so the 16-byte-aligned access faults on an unlucky frame base the runtime pin does not hit | tier: High
#[test]
fn bg_arg_shape_with_number_field_frame_region_is_aligned_in_ir() {
    // The marker is a claim about what codegen EMITS, so the unoptimized tier is pinned:
    // `default<O2>` legally renames or folds local value names.
    let built = build(FIXTURE, true);
    let ir = std::fs::read_to_string(&built.ir).expect("emitted IR must be readable");
    assert!(
        ir.contains("%scene_frame_region_aligned"),
        "the spawner's frame-embedded `scene` region must be wired through the aligning producer \
         (`scene_frame_region_aligned`); IR:\n{ir}"
    );
    assert!(
        ir.contains("%scene_frame_region_mask = and i64"),
        "the region pointer must be rounded with the FRAME_SHAPE_REGION_ALIGN mask; IR:\n{ir}"
    );
    assert!(
        ir.contains("%bg_shape_src = load"),
        "the spawn-site whole-struct copy (`bg_shape_src`) must still be emitted: this fixture \
         must keep exercising the inferred-copy path; IR:\n{ir}"
    );
}
