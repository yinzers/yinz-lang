// False-sharing padded-set analysis + the two v0.3-M4 Phase 4 lint rules
// (`cross-thread-fields-not-padded`, `prefer-yielding-sleep`).
//
// WHY: the padded set is the ONE derivation codegen layout and frame sizing thread —
// a wrong partition here silently forfeits cache-line isolation (module-local shape
// missed) or corrupts cross-module ABI (exported/imported shape padded in only one
// object file). The lints are the teaching surface for the decline bucket and the
// blocking-sleep nudge; both must carry three-part text and the registry rule name
// as their diagnostic code, and neither may ever be an error.

use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

const FILE: &str = "false_sharing_test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn lint_diags<'a>(out: &'a CheckOutput, rule: &str) -> Vec<&'a ynz_diagnostics::Diagnostic> {
    out.diagnostics
        .iter()
        .filter(|d| matches!(&d.kind, Some(DiagnosticKind::LintRule { rule: r }) if *r == rule))
        .collect()
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
const LOCAL_CROSSING: &str = "shape Tally { hits: int\n outs: int }\n\
     function record(give t: Tally) -> nothing { wait sleep(1)\n print(t.hits.toString()) }\n\
     function entrypoint() -> nothing { let t: Tally = { hits: 3, outs: 1 }\n background record(t) }";

#[test]
fn module_local_crossing_shape_lands_in_padded_set_with_no_lint() {
    // WHY: the transform's positive case — a module-local, non-exported shape whose
    // values cross a spawn boundary MUST be padded (codegen reads exactly this set),
    // and padding it silently is correct: the decline lint is for shapes that CANNOT
    // be padded, never for ones that were.
    let out = run(LOCAL_CROSSING);
    assert_no_errors(&out);
    assert!(
        out.typed_module
            .cross_thread_padded_shapes
            .contains("Tally"),
        "module-local crossing shape must be in the padded set; got: {:?}",
        out.typed_module.cross_thread_padded_shapes
    );
    assert!(
        lint_diags(&out, "cross-thread-fields-not-padded").is_empty(),
        "a PADDED shape must not fire the not-padded lint"
    );
}

#[test]
fn exported_crossing_shape_declines_padding_and_fires_lint() {
    // WHY: each module compiles to its own object file; padding an exported shape in
    // only the spawning module would give one program two layouts for one type — a
    // silent-memory-corruption class. The decline must be LOUD (the lint) and the
    // shape must stay out of the padded set.
    let src = LOCAL_CROSSING.replacen("shape Tally", "export shape Tally", 1);
    let out = run(&src);
    assert_no_errors(&out);
    assert!(
        !out.typed_module
            .cross_thread_padded_shapes
            .contains("Tally"),
        "exported shape must NOT be padded (cross-module layout divergence)"
    );
    let lints = lint_diags(&out, "cross-thread-fields-not-padded");
    assert_eq!(
        lints.len(),
        1,
        "exactly one decline lint per shape; got: {lints:#?}"
    );
    let lint = lints[0];
    assert_eq!(
        lint.severity,
        Severity::Suggestion,
        "the decline lint is a Tier 3 suggestion, never an error"
    );
    // Three-part teaching text, contextual to THIS shape and THIS reason.
    assert!(
        lint.what.contains("`Tally`"),
        "WHAT names the shape: {}",
        lint.what
    );
    assert!(!lint.what_instead.is_empty());
    assert!(
        lint.why.contains("it is exported"),
        "WHY carries the decline reason: {}",
        lint.why
    );
    assert!(
        lint.why.contains("cache line"),
        "WHY teaches the mechanism: {}",
        lint.why
    );
}

#[test]
fn non_crossing_shape_is_never_padded() {
    // WHY: padding costs memory (64 bytes per field) — a shape that never crosses a
    // spawn boundary must keep its natural layout even when background exists nearby.
    let src = "shape Tally { hits: int\n outs: int }\n\
         function ping() -> nothing { wait sleep(1) }\n\
         function entrypoint() -> nothing { let t: Tally = { hits: 3, outs: 1 }\n\
         background ping()\n print(t.hits.toString()) }";
    let out = run(src);
    assert_no_errors(&out);
    assert!(
        out.typed_module.cross_thread_padded_shapes.is_empty(),
        "no shape crosses the spawn — padded set must be empty; got: {:?}",
        out.typed_module.cross_thread_padded_shapes
    );
    assert!(lint_diags(&out, "cross-thread-fields-not-padded").is_empty());
}

#[test]
fn sleep_blocking_fires_prefer_yielding_sleep_suggestion() {
    // WHY: the roadmap-mandated Tier 3 nudge — dismissable, never an error, with the
    // user's own milliseconds echoed into the WHAT-INSTEAD (Golden Rule 11 context).
    let src = "function entrypoint() -> nothing { sleepBlocking(25) }";
    let out = run(src);
    assert_no_errors(&out);
    let lints = lint_diags(&out, "prefer-yielding-sleep");
    assert_eq!(lints.len(), 1, "one sleepBlocking call → one lint");
    let lint = lints[0];
    assert_eq!(
        lint.severity,
        Severity::Suggestion,
        "prefer-yielding-sleep is a dismissable suggestion, NEVER an error"
    );
    assert!(
        lint.what_instead.contains("wait sleep(25)"),
        "WHAT-INSTEAD echoes the literal ms: {}",
        lint.what_instead
    );
    assert!(!lint.why.is_empty());
}

#[test]
fn kernel_mode_does_not_fire_prefer_yielding_sleep() {
    // WHY: kernel mode has no scheduler to yield to — `sleepBlocking` is the RIGHT
    // call there (the design doc's kernel row steers TOWARD it); a lint would be a
    // false positive nagging correct code.
    let src = "function entrypoint() -> nothing { sleepBlocking(25) }";
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), src.to_string());
    let parse = ynz_parser::parse_query(&db, sf);
    let sig_output = ynz_typeck::queries::module_signatures_query(&db, sf);
    let (typed, _mono, check_diags) = ynz_typeck::check_with_kernel_mode(
        &parse.module,
        &sig_output.sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &ynz_typeck::intrinsics::PrimitiveIntrinsicTable::m6(),
        &sig_output.imported_options,
    );
    let fired = check_diags.iter().any(
        |d| matches!(&d.kind, Some(DiagnosticKind::LintRule { rule }) if *rule == "prefer-yielding-sleep"),
    );
    assert!(!fired, "kernel mode must not fire prefer-yielding-sleep");
    // Kernel mode also never pads (background is a compile error there).
    assert!(typed.cross_thread_padded_shapes.is_empty());
}
