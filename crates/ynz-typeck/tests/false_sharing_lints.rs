// False-sharing padded-set analysis (v0.3-M4 Phase 4).
//
// WHY: the padded set is the ONE derivation codegen layout and frame sizing thread —
// a wrong partition here silently forfeits cache-line isolation (module-local shape
// missed) or corrupts cross-module ABI (exported/imported shape padded in only one
// object file).

use ynz_diagnostics::Severity;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

const FILE: &str = "false_sharing_test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_no_errors(out: &CheckOutput) {
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect();
    assert!(errors.is_empty(), "expected 0 errors, got: {errors:#?}");
}

// A module-local shape given across a `background` spawn — the paddable case.
const LOCAL_CROSSING: &str = "shape Tally { hits: Int\n outs: Int }\n\
     function record(give t: Tally) -> Nothing { wait sleep(1)\n print(t.hits.toString()) }\n\
     function entrypoint() -> Nothing { let t: Tally = { hits: 3, outs: 1 }\n background record(t) }";

// A shape that exists next to a `background` call but never crosses the spawn.
const NON_CROSSING: &str = "shape Tally { hits: Int\n outs: Int }\n\
         function ping() -> Nothing { wait sleep(1) }\n\
         function entrypoint() -> Nothing { let t: Tally = { hits: 3, outs: 1 }\n\
         background ping()\n print(t.hits.toString()) }";

// bug: typeck pads an exported cross-thread shape in only the spawning module (two layouts for one type across object files, memory corruption) | tier: High
#[test]
fn exported_crossing_shape_declines_padding() {
    // Each module compiles to its own object file; padding an exported shape in only
    // the spawning module would give one program two layouts for one type.
    let src = LOCAL_CROSSING.replacen("shape Tally", "export shape Tally", 1);
    let out = run(&src);
    assert_no_errors(&out);
    assert!(
        !out.typed_module
            .cross_thread_padded_shapes
            .contains("Tally"),
        "exported shape must NOT be padded (cross-module layout divergence)"
    );
}

macro_rules! padded_set_cases {
    ($($case:ident: $source:expr, $is_padded:expr;)+) => {
        $(
            // bug: typeck pads a shape that never crosses a spawn, or leaves a module-local crossing shape unpadded (extra memory or lost cache-line isolation, output unchanged) | tier: Medium
            #[test]
            fn $case() {
                let out = run($source);
                assert_no_errors(&out);
                assert_eq!(
                    out.typed_module.cross_thread_padded_shapes.contains("Tally"),
                    $is_padded,
                    "padded set was: {:?}",
                    out.typed_module.cross_thread_padded_shapes
                );
            }
        )+
    };
}

padded_set_cases! {
    module_local_crossing_shape_lands_in_padded_set: LOCAL_CROSSING, true;
    non_crossing_shape_is_never_padded: NON_CROSSING, false;
}
