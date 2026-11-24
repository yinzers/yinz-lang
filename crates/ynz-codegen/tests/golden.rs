// Codegen integration tests through the public `codegen_query`: determinism, which valid programs
// compile, the runtime calls the emitted IR routes to, the compiled state machine of a `wait`,
// and the spawn count agreeing with the CPU-group admission authority.

use std::path::PathBuf;

use ynz_codegen::{codegen_query, sha256};
use ynz_parser::{CompilerDb, SourceFile};

/// Diagnostics that gate a clean compile: everything EXCEPT Suggestion severity.
/// Tier 3 lint suggestions (v0.3-M4 `[[lint_rule]]`, e.g. `prefer-yielding-sleep` on
/// the deliberate sleepBlocking keepalives several fixtures here use) are
/// informational teaching surfaces — they never block compilation and must not fail
/// a "compiles clean" gate. Mirrors the assert_clean ratchet in ynz-typeck/tests.
fn gating_diags(bucket: &ynz_diagnostics::DiagnosticBucket) -> Vec<&ynz_diagnostics::Diagnostic> {
    bucket
        .iter()
        .filter(|diagnostic| diagnostic.severity != ynz_diagnostics::Severity::Suggestion)
        .collect()
}

const NON_CROSSING_LOCAL_FILE: &str = "v0_3_m3a_p1_non_crossing_local_not_slotted.ynz";

/// Read the non-crossing-local fixture source from disk.
///
/// The fixture file is the single source of truth for this program. Reading it directly
/// avoids maintaining a parallel inline copy that can silently diverge if the fixture
/// changes — one definition, one place to update.
fn non_crossing_local_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ynz-driver/tests/fixtures")
        .join(NON_CROSSING_LOCAL_FILE);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

const HELLO_FILE: &str = "hello.ynz";
// test-ratchet: M7 P1 migrates double-quoted string to backtick syntax — double-quotes now
// produce an error diagnostic, so any test source must use backticks.
const HELLO_SOURCE: &str = "function entrypoint() -> Nothing { print(`hello, yinz`) }";

const ARITHMETIC_SOURCE: &str = r#"function entrypoint() -> Nothing {
  let price = 0.1 + 0.2
  let count: Int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;

const FIB_SOURCE: &str = r#"function fib(n: Int) -> Int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
function entrypoint() -> Nothing {
  let result = fib(10)
  print(result)
}"#;

// test-ratchet: M7 P1 migrates double-quoted string to backtick syntax
const PLAYER_SOURCE: &str = "shape Player {\n  name: String\n  health: Int\n}\nfunction greet(share self: Player) -> Nothing {\n  print(self.name)\n}\nfunction heal(lend self: Player, amount: Int) -> Nothing {\n  self.health = self.health + amount\n}\nfunction consume(give p: Player) -> Nothing {\n  print(p.name)\n}\nfunction entrypoint() -> Nothing {\n  let p: Player = { name: `Patrick`, health: 100 }\n  p.greet()\n  p.heal(20)\n  print(p.health.toString())\n  consume(p)\n}";

/// Compile `source` and return its IR text. Panics when codegen reports a gating diagnostic or
/// emits an empty object.
fn run_m2_sm_codegen(file: &str, source: &str) -> String {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, file.to_string(), source.to_string());
    let output = codegen_query(&db, source_file);
    assert!(
        gating_diags(&output.diagnostics).is_empty(),
        "source `{file}` must compile without diagnostics:\n{:#?}",
        output.diagnostics
    );
    assert!(
        !output.artifact.object_bytes.is_empty(),
        "source `{file}` must produce a non-empty object"
    );
    output.artifact.ir_text.clone()
}

// bug: the SHA-256 helper returns a wrong digest, so the artifact hash that proves two compiles identical is meaningless | tier: High
#[test]
fn sha256_of_empty_input_matches_known_value() {
    // FIPS test vector for the empty message: a block of padding only.
    let expected = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(sha256(b""), expected);
}

// bug: the SHA-256 helper returns a wrong digest for a message shorter than one block, so the artifact hash that proves two compiles identical is meaningless | tier: High
#[test]
fn sha256_of_abc_matches_known_value() {
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(sha256(b"abc"), expected);
}

// bug: two compiles of the same source produce different object bytes (a map iterated in hash order, an address in the output), so builds are not reproducible | tier: Medium
#[test]
fn codegen_is_deterministic_across_two_runs() {
    let first_db = CompilerDb::default();
    let first_source = SourceFile::new(&first_db, HELLO_FILE.to_string(), HELLO_SOURCE.to_string());
    let first = codegen_query(&first_db, first_source).artifact.clone();

    let second_db = CompilerDb::default();
    let second_source =
        SourceFile::new(&second_db, HELLO_FILE.to_string(), HELLO_SOURCE.to_string());
    let second = codegen_query(&second_db, second_source).artifact.clone();

    assert_eq!(first.sha256, second.sha256);
    assert_eq!(first.object_bytes, second.object_bytes);
}

/// One named test per program so the runner names the failing case. Four cases, each a different
/// lowering path (a literal print, decimal arithmetic, recursion, shape parameters with ownership
/// modifiers), each with its own way to fail: a diagnostic on a valid program, or an empty object.
macro_rules! valid_program_case {
    ($test_name:ident, file: $file:expr, source: $source:expr) => {
        // bug: codegen rejects a valid program or LLVM silently emits an empty object file | tier: Medium
        #[test]
        fn $test_name() {
            let db = CompilerDb::default();
            let source_file = SourceFile::new(&db, $file.to_string(), $source.to_string());
            let output = codegen_query(&db, source_file);
            assert!(
                gating_diags(&output.diagnostics).is_empty(),
                "{:#?}",
                output.diagnostics
            );
            assert!(!output.artifact.object_bytes.is_empty());
        }
    };
}

valid_program_case!(a_hello_program_compiles_to_an_object, file: HELLO_FILE, source: HELLO_SOURCE);
valid_program_case!(decimal_arithmetic_compiles_to_an_object, file: "m2_smoke.ynz", source: ARITHMETIC_SOURCE);
valid_program_case!(a_recursive_function_compiles_to_an_object, file: "m3_fib.ynz", source: FIB_SOURCE);
valid_program_case!(shape_parameters_with_ownership_modifiers_compile_to_an_object, file: "m4_player.ynz", source: PLAYER_SOURCE);

// bug: a `share` parameter loses its `readonly` and `noalias` attributes, so the optimizer cannot assume it is not written and the design's performance invariant is lost silently | tier: Medium
#[test]
fn a_share_parameter_is_readonly_and_noalias_in_the_ir() {
    let ir = run_m2_sm_codegen("m4_player.ynz", PLAYER_SOURCE);
    assert!(ir.contains("readonly") && ir.contains("noalias"));
}

// bug: a local read only before a `wait` is given a frame slot (every local slotted, not only the ones that cross a suspension), growing every state-machine frame | tier: Medium
#[test]
fn non_crossing_local_is_not_frame_slotted() {
    // Frame-slot stores are named `ls_{slot_index}` (`store_local_slot`); the fixture's `setup`
    // is read only before its wait, so no slot store may appear.
    let ir = run_m2_sm_codegen(NON_CROSSING_LOCAL_FILE, &non_crossing_local_source());
    assert!(
        !ir.contains("ls_0") && !ir.contains("ls_1"),
        "a non-crossing local was frame-slotted (`flush_crossing_local_if_needed` slots a local \
         that is never read after a wait)"
    );
}

/// One named test per construct so the runner names the failing case. Each compiled state
/// machine must carry its resume function and the runtime calls its waits lower to.
macro_rules! state_machine_case {
    ($test_name:ident, file: $file:expr, source: $source:expr, check: $check:expr) => {
        // bug: a `wait` is lowered to nothing, or the state machine loses a resume point or a live suspend path, so a function that waits runs straight through | tier: High
        #[test]
        fn $test_name() {
            let ir = run_m2_sm_codegen($file, $source);
            let check: fn(&str) = $check;
            check(&ir);
        }
    };
}

// Case bug: a single wait has no resume function or no sleep create/poll pair.
state_machine_case!(
    a_single_wait_emits_a_resume_function_and_polls_the_sleep,
    file: "v03m2_single_wait.ynz",
    source: r#"
function pause() -> Nothing {
  wait sleep(100)
}
function entrypoint() -> Nothing {
  background pause()
  sleepBlocking(200)
}
"#,
    check: |ir| {
        assert!(ir.contains("ynz_sm_pause_resume"));
        assert!(ir.contains("ynz_rt_async_sleep_create"));
        assert!(ir.contains("ynz_rt_async_sleep_poll"));
    }
);
// Case bug: the second of two sequential waits shares or loses the first one's resume point.
state_machine_case!(
    two_sequential_waits_emit_two_sleep_creates,
    file: "v03m2_multi_wait.ynz",
    source: r#"
function chain() -> Nothing {
  wait sleep(50)
  wait sleep(50)
}
function entrypoint() -> Nothing {
  background chain()
  sleepBlocking(200)
}
"#,
    check: |ir| {
        assert!(ir.contains("ynz_sm_chain_resume"));
        let sleep_create_count = ir.matches("ynz_rt_async_sleep_create").count();
        assert!(sleep_create_count >= 2, "got {sleep_create_count} sleep creates");
    }
);
// Case bug: a wait inside an `if` is silently no-oped, or its suspend block has no predecessor.
state_machine_case!(
    a_wait_inside_an_if_polls_in_the_branch_and_the_suspend_path_is_live,
    file: "v03m2_wait_in_if.ynz",
    source: r#"
function maybeWait(b: Boolean) -> Nothing {
  if (b) {
    wait sleep(100)
  }
}
function entrypoint() -> Nothing {
  background maybeWait(true)
  sleepBlocking(200)
}
"#,
    check: |ir| {
        assert!(ir.contains("ynz_sm_maybeWait_resume"));
        assert!(
            ir.contains("ynz_rt_async_sleep_poll"),
            "the wait inside the branch was silently no-oped"
        );
        let suspend_block_line = ir.lines().find(|line| line.contains("sm_pending:")).unwrap_or("");
        assert!(
            !suspend_block_line.contains("No predecessors!"),
            "the suspend path is dead code"
        );
    }
);

/// One named test per construct so the runner names the failing case. Five cases, each a
/// different emitter path that decides which runtime call a construct lowers to.
macro_rules! runtime_call_routing_case {
    ($test_name:ident, file: $file:expr, source: $source:expr, check: $check:expr) => {
        // bug: a construct lowers to the wrong runtime call (no preempt check in a loop, a blocking-pool spawn for a waiting function, a missing entrypoint driver or runtime init), so a program starves, ties up OS threads or panics at start | tier: Medium
        #[test]
        fn $test_name() {
            let ir = run_m2_sm_codegen($file, $source);
            let check: fn(&str) = $check;
            check(&ir);
        }
    };
}

/// Number of `call` instructions in `ir` naming `callee` exactly (not a longer symbol sharing the
/// prefix).
fn call_count(ir: &str, callee: &str) -> usize {
    let call_target = format!("@{callee}(");
    ir.lines()
        .filter(|line| line.contains("call") && line.contains(&call_target))
        .count()
}

// Case bug: a `while` back-edge has no preempt check, so a loop starves the scheduler.
runtime_call_routing_case!(
    a_while_loop_back_edge_checks_for_preemption,
    file: "v03m1while.ynz",
    source: r#"
function entrypoint() -> Nothing {
  let x: Int = 3
  while (x > 0) {
    x = x - 1
  }
  print(x)
}
"#,
    check: |ir| assert!(ir.contains("ynz_rt_check_preempt"))
);
// Case bug: `background` of a waiting function takes the blocking pool and ties up an OS thread.
runtime_call_routing_case!(
    background_of_a_waiting_function_spawns_on_the_io_pool,
    file: "v03m2_bg_spawn_sm.ynz",
    source: r#"
function worker() -> Nothing {
  wait sleep(100)
}
function entrypoint() -> Nothing {
  background worker()
  sleepBlocking(200)
}
"#,
    check: |ir| {
        assert!(call_count(ir, "ynz_rt_spawn") > 0);
        assert_eq!(call_count(ir, "ynz_rt_spawn_blocking"), 0);
    }
);
// Case bug: `background` of a wait-free function stops using the blocking pool.
runtime_call_routing_case!(
    background_of_a_wait_free_function_spawns_on_the_blocking_pool,
    file: "v03m2_bg_regular.ynz",
    source: r#"
function worker() -> Nothing {
  sleepBlocking(100)
}
function entrypoint() -> Nothing {
  background worker()
  sleepBlocking(200)
}
"#,
    check: |ir| assert!(call_count(ir, "ynz_rt_spawn_blocking") > 0)
);
// Case bug: a plain function calling a waiting one does not run under the program-entry driver.
runtime_call_routing_case!(
    a_plain_caller_of_a_waiting_function_runs_the_entrypoint_driver,
    file: "v03m2_non_sm_caller.ynz",
    source: r#"
function sleeper() -> Nothing {
  wait sleep(100)
}
function entrypoint() -> Nothing {
  sleeper()
}
"#,
    check: |ir| assert!(ir.contains("ynz_rt_run_entrypoint"))
);
// Case bug: `main` runs the entrypoint driver before `ynz_rt_init`, so the program panics with
// "ynz_rt_init not called".
runtime_call_routing_case!(
    main_initializes_the_runtime_before_running_a_waiting_entrypoint,
    file: "v03m2_rt_init_first.ynz",
    source: r#"
function entrypoint() -> Nothing {
  wait sleep(1)
}
"#,
    check: |ir| {
        let mut in_main = false;
        let mut runtime_initialized = false;
        let mut driver_ran_before_init = false;
        let mut driver_ran = false;
        for line in ir.lines() {
            if line.contains("define") && line.contains("@main") {
                in_main = true;
            }
            if in_main {
                if line.contains("@ynz_rt_init") {
                    runtime_initialized = true;
                }
                if line.contains("@ynz_rt_run_entrypoint") {
                    driver_ran = true;
                    driver_ran_before_init |= !runtime_initialized;
                }
                if line == "}" {
                    break;
                }
            }
        }
        assert!(runtime_initialized, "main must call ynz_rt_init");
        assert!(driver_ran, "main must call ynz_rt_run_entrypoint");
        assert!(!driver_ran_before_init, "ynz_rt_run_entrypoint appears before ynz_rt_init");
    }
);

const TWO_CPU_GROUPS_SOURCE: &str = r#"
function fib(n: Int) -> Int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function entrypoint() -> Nothing {
  let a = fib(10)
  let b = fib(11)
  print(a)
  print(b)
  if (a > 0) {
    let c = fib(12)
    let d = fib(13)
    print(c)
    print(d)
  }
}
"#;

/// Number of `ynz_rt_spawn_blocking_joinable` call instructions in `ir` — one per spawned CPU
/// group member.
fn joinable_spawn_count(ir: &str) -> usize {
    ir.lines()
        .filter(|l| l.contains("call") && l.contains("ynz_rt_spawn_blocking_joinable"))
        .count()
}

/// The admitted CPU group for the named function in `source`, computed exactly the way the
/// inlay-hint pass and codegen compute it (effective suspend set + CPU-ABI supported callees).
/// Returns `None` when the authority declines to spike-host the function.
fn admitted_group_for(
    source: &str,
    fn_name: &str,
) -> Option<ynz_typeck::cpu_admission::AdmittedCpuGroup> {
    use ynz_typeck::cpu_admission::admitted_cpu_group;
    use ynz_typeck::queries::{check_query, module_signatures_query};
    use ynz_typeck::signatures::build_effective_suspend_set;

    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        format!("/tmp/ynz_xpin_{fn_name}.ynz"),
        source.to_string(),
    );

    let sig_output = module_signatures_query(&db, sf);
    let check_out = check_query(&db, sf);
    let effective_suspends =
        build_effective_suspend_set(&check_out.suspends_set, &sig_output.imported_fns);
    let supported: std::collections::HashSet<String> = sig_output
        .sig_table
        .fns
        .iter()
        .filter(|(_, sig)| ynz_typeck::independence::cpu_result_abi_supports(&sig.ret))
        .map(|(name, _)| name.clone())
        .collect();

    let parse = ynz_parser::parse_query(&db, sf);
    let f = parse.module.items.iter().find_map(|it| match it {
        ynz_ast::nodes::Item::Function(f) if f.name == fn_name => Some(f),
        _ => None,
    })?;
    admitted_cpu_group(
        f,
        &effective_suspends,
        &supported,
        &check_out.typed_module.expr_types,
    )
}

const XPIN_FIRE_SOURCE: &str = r#"
function fib(n: Int) -> Int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function entrypoint() -> Nothing {
  let a = fib(20)
  let b = fib(21)
  print(a)
  print(b)
}
"#;

// bug: codegen spawns a different number of CPU-group members than the admission authority admitted, so the IDE hint and the binary disagree and a second group can alias the single reserved slot region | tier: High
#[test]
fn cross_pin_fire_spawn_count_equals_admitted_member_count() {
    // WHY: pins the BINARY's emitted spawn count directly to the single admission AUTHORITY
    //      (`admitted_cpu_group`), not to a hardcoded literal. The hint side is already pinned to
    //      the authority (ynz-typeck/tests/parallel_group_hint_parity.rs); without this the codegen
    //      side only asserted hand-written `expected_spawns` numbers, so a unification bug that
    //      silently changed which members spawn could go green. The invariant: for an admitted
    //      function, codegen emits exactly one `ynz_rt_spawn_blocking_joinable` per admitted group
    //      member. If you're tempted to relax `== members.len()` to a tolerance or a literal, the
    //      bug is a spike-admission/codegen divergence — fix that, not this assertion.
    let ir = run_m2_sm_codegen("xpin_fire.ynz", XPIN_FIRE_SOURCE);
    let spawns = joinable_spawn_count(&ir);

    let group =
        admitted_group_for(XPIN_FIRE_SOURCE, "entrypoint").expect("entrypoint must be admitted");
    assert_eq!(
        spawns,
        group.member_indices.len(),
        "emitted joinable-spawn count ({spawns}) must equal the authority's admitted member count \
         ({}); a mismatch is a spike-admission ⇄ codegen divergence (the exact hint↔binary \
         silent-drift class). IR:\n{ir}",
        group.member_indices.len()
    );
    // Sanity: a clean 2-member group fires 2 spawns. If this changes, the fixture changed, not the
    // invariant — the assertion above is the real guard.
    assert_eq!(spawns, 2, "clean 2-member group must spawn exactly 2");
}

// bug: a function the admission authority declined (two CPU groups) still spawns, so the hint says sequential while the binary runs a group on another thread | tier: High
#[test]
fn cross_pin_decline_means_zero_spawns() {
    // WHY: the DECLINE half of the cross-pin invariant — `admitted_cpu_group(...) == None` MUST
    //      coincide with ZERO emitted spawns. Together with the FIRE test this makes the binary's
    //      spawn behavior a direct function of the admission authority in BOTH directions, closing
    //      the gap the prior hardcoded-`expected_spawns` golden tests left open. A two-CPU-group
    //      function is the canonical decline (single-group constraint). If this fails, an admission
    //      change started spawning for a declined function — fix the divergence, not this test.
    let ir = run_m2_sm_codegen("xpin_decline.ynz", TWO_CPU_GROUPS_SOURCE);
    let spawns = joinable_spawn_count(&ir);

    let group = admitted_group_for(TWO_CPU_GROUPS_SOURCE, "entrypoint");
    assert!(
        group.is_none(),
        "two-CPU-group function must be DECLINED by the authority; got {group:?}"
    );
    assert_eq!(
        spawns, 0,
        "a declined function (admitted_cpu_group == None) must emit ZERO spawns; got {spawns}. \
         IR:\n{ir}"
    );
}
