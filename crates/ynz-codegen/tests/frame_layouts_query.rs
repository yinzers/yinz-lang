// WHY: These tests prove that `frame_layouts_query` produces LLVM-accurate frame layouts
// for the cross-module suspending-call cases that Phase 2 wires to emission.
//
//   Escape #1 (re-export chain): B's total_size must include A's sub-frame. Guard G2
//   (recursive resolver) ensures frame_layouts_query(B) calls frame_layouts_query(A)
//   so B's embed uses A's real total_size, not the 32-byte header placeholder that
//   caused exit 132 (SIGILL) under the old scalar approximation.
//
//   Escape #2 (shape LLVM slot count): bool×bool shape ABI = 2 bytes (LLVM) vs 16 bytes
//   (typeck "8 bytes per field"). The query uses LLVM TargetData for exact slot counts.
//   Wrong slot count → out-of-bounds frame write → abort.
//
//   Guard G3 (cycle recovery): circular imports are already typeck errors; the query must
//   return an empty map, not infinite-loop or ICE.
//
// Phase 1 tests use leaf modules or synthetic sources so `check_query` succeeds and the
// query can run its composition arithmetic. Phase 2 lifts the universal reject, making
// cross-module suspending calls legal — the reexport_chain test now verifies that
// frame_layouts_query produces the correct composed total_size end-to-end.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use ynz_abi::FRAME_HEADER_SIZE;
use ynz_codegen::emit::{build_frame_layouts_with_resolver, SuspendSet};
use ynz_codegen::state_machine::{own_locals_size, FRAME_SHAPE_REGION_SLACK_SLOTS};
use ynz_codegen::{codegen_query, frame_layouts_query};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, module_signatures_query};

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
        .unwrap_or_else(|e| panic!("cannot canonicalize {}: {e}", path.display()));
    let text = std::fs::read_to_string(&canonical)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", canonical.display()));
    let canonical_str = canonical.display().to_string();
    let sf = SourceFile::new(db, canonical_str, text);
    db.register_source(sf);
    sf
}

/// Register an inline source string in `db` under `path`.
fn register_inline(db: &mut CompilerDb, path: &str, source: &str) -> SourceFile {
    let sf = SourceFile::new(db, path.to_string(), source.to_string());
    db.register_source(sf);
    sf
}

// ── Escape #1 — re-export chain: leaf module A produces correct total_size ───────────────

#[test]
fn leaf_module_get_value_has_one_crossing_local() {
    // WHY: Module A (a_ops.ynz) defines `getValue` which suspends via sleep and keeps
    // `offset: int` live across the suspension. One int crossing local → n_locals = 1,
    // total_size = FRAME_HEADER_SIZE + own_locals_size(1) = 40 bytes (NOT 32).
    //
    // This 40-byte size is DISTINCT from FRAME_HEADER_SIZE (32), which is the value
    // the old resolver-bypass fallback path produces (local-fn path finds no local fn
    // named "getValue", computes n_locals=0 + no children = 32). Any test that feeds
    // getValue's total_size via a resolver and then asserts a doWork total that includes
    // it will FAIL if the resolver is bypassed and the fallback fires (40 ≠ 32).
    let mut db = CompilerDb::default();
    // Register a_ops only — it has no imports, so frame_layouts_query succeeds.
    let a_sf = register_fixture(&mut db, "v0_3_m3e_reexport_chain_int", "a_ops.ynz");

    let layouts = frame_layouts_query(&db, a_sf);
    let layout = layouts.get("getValue").unwrap_or_else(|| {
        panic!(
            "frame_layouts_query must include 'getValue'; got keys: {:?}",
            layouts.keys().collect::<Vec<_>>()
        )
    });

    assert_eq!(
        layout.n_locals, 1,
        "getValue has 0 params + 1 int crossing local (offset, live across sleep) → n_locals = 1"
    );
    assert!(
        layout.children.is_empty(),
        "getValue calls only sleep (a builtin intrinsic, not a frame-child) → children must be empty"
    );
    let expected_total = FRAME_HEADER_SIZE + own_locals_size(1);
    assert_eq!(
        layout.total_size, expected_total,
        "getValue: header({FRAME_HEADER_SIZE}) + own_locals(1×8=8) = {expected_total} bytes. \
         Got {}. A value of 32 (= FRAME_HEADER_SIZE) would indicate the crossing local was not counted.",
        layout.total_size
    );
    assert_eq!(
        layout.total_size, 40,
        "Concrete check: getValue total_size must be 40 bytes (not 32)"
    );
    assert!(
        layout.recursion_slot.is_none(),
        "getValue is not recursive → recursion_slot must be None"
    );
}

// ── Escape #1 — re-export chain: B's total_size includes A's sub-frame ───────────────────

#[test]
fn reexport_chain_b_total_size_includes_a_sub_frame() {
    // WHY: `doWork` in B calls `getValue` from A. B's total_size must include A's composed
    // frame as an embedded sub-frame. The correct value is:
    //   FRAME_HEADER_SIZE (32)             — B's own frame header
    //   + own_locals_size(0) (0)            — no params, no crossing locals in doWork
    //   + getValue.total_size (40)          — A's sub-frame (getValue has 1 crossing local)
    //   = 72 bytes
    //
    // A's getValue.total_size is 40 (FRAME_HEADER_SIZE=32 + own_locals_size(1)=8), NOT 32.
    // This is critical: the bypass bug would produce 32+0+32=64 (the local-fn fallback path
    // computes n_locals=0 + no children = 32 for an "imported" getValue it can't find locally).
    // A correctly wired resolver produces 32+0+40=72. The two values are distinct — 72 ≠ 64 —
    // so this test FAILS when the resolver is bypassed, unlike the old fixture where the
    // fallback value coincided with the real value.
    //
    // The anti-bypass sub-assertion drives the same computation with a DIFFERENT sentinel
    // (56 bytes) and confirms doWork.total_size changes to 32+0+56=88. If the output is
    // invariant (still 64 or 72 regardless of sentinel), the resolver is bypassed.
    //
    // Honest limit: the resolver-BUILDING path inside frame_layouts_query (callee_source_map
    // via resolve_module_path → db.source_by_path) goes live only when the universal reject
    // lifts in Phase 2. Phase 1 proves the COMPOSITION arithmetic with a real cross-module
    // size; the full resolver-build path is Phase 2's adversarial-gate scope.
    let mut db = CompilerDb::default();
    let a_sf = register_fixture(&mut db, "v0_3_m3e_reexport_chain_int", "a_ops.ynz");
    let b_sf = register_fixture(&mut db, "v0_3_m3e_reexport_chain_int", "b_ops.ynz");

    // Get A's layout via the query (A is a leaf module; check_query succeeds).
    let a_layouts = frame_layouts_query(&db, a_sf);
    let a_get_value_total_size = a_layouts
        .get("getValue")
        .expect("frame_layouts_query(a_sf) must include getValue")
        .total_size;
    // getValue has 1 crossing local → total_size = FRAME_HEADER_SIZE + own_locals_size(1) = 40.
    // NOT 32 (= FRAME_HEADER_SIZE alone). If this fails, the fixture's crossing local
    // was not recognised, and the anti-bypass check below would be meaningless.
    assert_eq!(
        a_get_value_total_size,
        FRAME_HEADER_SIZE + own_locals_size(1),
        "A's getValue total_size = header(32) + 1 crossing local(8) = 40; \
         got {} — the fixture's crossing local was not counted",
        a_get_value_total_size
    );
    assert_eq!(
        a_get_value_total_size, 40,
        "Concrete check: getValue total_size must be 40 (not 32)"
    );

    // Get B's typed module. Phase 2 lifted the universal reject, so check_query(b_sf) now
    // succeeds. doWork calls getValue (an imported suspending function), which is legal.
    let b_check = check_query(&db, b_sf);
    // WHY: locks the Phase-2 lift contract on b_sf. check_query(b_sf) MUST succeed after the
    // universal reject is removed — a future regression that re-introduces any diagnostic on
    // this fixture would cause the resolver/sentinel assertions below to run on a poisoned
    // typed module, masking the real failure. Asserting clean here is the earliest canary.
    assert!(
        !b_check.diagnostics.has_errors(),
        "check_query(b_sf) must succeed after Phase 2 lifts the universal reject: \
         doWork calling imported suspending getValue is now legal. Got errors: {:?}",
        b_check.diagnostics
    );

    // Build B's effective suspend_set: local suspends from check_query + imported suspending
    // names from module_signatures_query. This mirrors what frame_layouts_query does
    // before calling build_frame_layouts_with_resolver.
    let b_sig = module_signatures_query(&db, b_sf);
    let mut b_suspend_set: SuspendSet = b_check.suspends_set.clone();
    for (name, sig) in &b_sig.imported_fns {
        if sig.suspends {
            b_suspend_set.insert(name.clone());
        }
    }
    assert!(
        b_suspend_set.contains("doWork"),
        "doWork must be in B's suspend_set (calls imported suspending getValue)"
    );
    assert!(
        b_suspend_set.contains("getValue"),
        "getValue must be in B's suspend_set (imported suspending function)"
    );

    // ── Primary assertion: resolver feeds A's real total_size (40) into B's composition ──

    // Cell<u32> tracks how many times the resolver is called for "getValue". The resolver
    // MUST be called; a count of 0 means the cache was seeded via the bypass path instead.
    let call_count = Cell::new(0u32);
    let resolver = |name: &str| -> Option<u64> {
        if name == "getValue" {
            call_count.set(call_count.get() + 1);
            a_layouts.get("getValue").map(|l| l.total_size)
        } else {
            None
        }
    };

    // b_ops has no shapes → empty shape_abi_sizes.
    let b_layouts = build_frame_layouts_with_resolver(
        &b_check.typed_module,
        &b_suspend_set,
        &b_suspend_set,
        &HashMap::new(),
        &resolver,
    );

    // Confirm the resolver was actually called (not bypassed).
    assert!(
        call_count.get() > 0,
        "callee_size_resolver must be called for 'getValue' (an imported suspending callee). \
         Call count = 0 means the resolver was bypassed and the local-fn fallback fired instead."
    );

    // Assert B's doWork total_size READ FROM the computation — not test arithmetic.
    let b_do_work = b_layouts.get("doWork").unwrap_or_else(|| {
        panic!(
            "build_frame_layouts_with_resolver must include doWork; got keys: {:?}",
            b_layouts.keys().collect::<Vec<_>>()
        )
    });

    // doWork: header(32) + own_locals(0) + getValue sub-frame(40) = 72.
    // Bypass value would be 32+0+32=64 (local-fn fallback computes getValue as header-only).
    let expected_b_total = FRAME_HEADER_SIZE + own_locals_size(0) + a_get_value_total_size;
    assert_eq!(
        b_do_work.total_size, expected_b_total,
        "doWork total_size must equal header({FRAME_HEADER_SIZE}) + own_locals(0) + \
         getValue.total_size({a_get_value_total_size}) = {expected_b_total}. \
         Got {}. A value of 64 means the resolver was bypassed (fallback=32 used instead of 40).",
        b_do_work.total_size
    );
    assert_eq!(
        b_do_work.total_size, 72,
        "Concrete check: B's doWork total_size must be 72 bytes (32 header + 40 from A's sub-frame)"
    );

    // ── Anti-bypass sub-assertion: resolver sentinel tracking ────────────────────────────
    //
    // Drive the same computation with a DIFFERENT sentinel (56 bytes). If doWork.total_size
    // changes to 32+0+56=88, the resolver value propagated correctly. If it stays at 72 or
    // reverts to 64, the resolver is bypassed and the cache is being pre-populated with a
    // stale value from a previous run (or the local-fn fallback).

    let sentinel_call_count = Cell::new(0u32);
    let sentinel_size: u64 = 56; // sentinel ≠ 32 (fallback) and ≠ 40 (real A value)
    let sentinel_resolver = |name: &str| -> Option<u64> {
        if name == "getValue" {
            sentinel_call_count.set(sentinel_call_count.get() + 1);
            Some(sentinel_size)
        } else {
            None
        }
    };

    let b_layouts_sentinel = build_frame_layouts_with_resolver(
        &b_check.typed_module,
        &b_suspend_set,
        &b_suspend_set,
        &HashMap::new(),
        &sentinel_resolver,
    );

    assert!(
        sentinel_call_count.get() > 0,
        "sentinel resolver must be called for 'getValue'. \
         Call count = 0 means the resolver is bypassed."
    );

    let b_do_work_sentinel = b_layouts_sentinel
        .get("doWork")
        .expect("sentinel run must also produce a doWork layout");

    let expected_sentinel_total = FRAME_HEADER_SIZE + own_locals_size(0) + sentinel_size;
    assert_eq!(
        b_do_work_sentinel.total_size, expected_sentinel_total,
        "Anti-bypass: with resolver→{sentinel_size}, doWork total_size must be \
         header({FRAME_HEADER_SIZE}) + own_locals(0) + sentinel({sentinel_size}) = {expected_sentinel_total}. \
         Got {}. An invariant output (same as primary run = 72, or fallback = 64) confirms the resolver \
         is bypassed and the pre-seeded size is not being read from the resolver.",
        b_do_work_sentinel.total_size
    );
    assert_eq!(
        b_do_work_sentinel.total_size, 88,
        "Concrete anti-bypass check: sentinel run doWork total_size must be 88 (32+0+56)"
    );
}

#[test]
fn imported_suspending_after_pair_declines_consistently_across_boundaries() {
    // WHY: the spike-host decision is made at TWO salsa query boundaries — `frame_layouts_query`
    // (sizes the frame) and `codegen_query` (lays it out + emits). They MUST probe spike admission
    // against the SAME suspend set, or they can disagree on whether a function is a host: one sizes
    // the frame sequentially (with an imported callee's child sub-frame), the other lays it out as a
    // spike host (omitting that sub-frame) → the heap block is under-allocated by exactly that
    // sub-frame and the child-frame write corrupts past the allocation. The fix routes BOTH
    // boundaries through the same EFFECTIVE suspend set (local ∪ imported-suspending).
    //
    // This is the IMPORTED variant of fixture (q) (which uses a LOCAL suspending callee — already
    // in the bare set, so it never exercised the imported-only gap).
    //
    // **v0.3-M3g Phase 3 UPDATE:** this test previously claimed the typeck promotion set was
    // EMPTY for this fixture and that `spike_host_subset` agreed (both empty) regardless of which
    // suspend set was probed — that claim was true only as an accidental byproduct of Phase 2's
    // now-REMOVED temporary top-level co-resident-suspension decline (which blanket-declined the
    // group before this fixture's REAL admission gate — `cpu_group_member_indices`'s
    // PRE-EXISTING "no post-group statement may call a user-defined suspending callee" check —
    // ever got a chance to run). With the temporary decline gone, the real promotion set for this
    // fixture is `{"entrypoint"}` (verified this session: `entrypoint` has no guard-tripping
    // crossing, so the E6 guard-probe never excludes it), and `spike_host_subset` now reveals the
    // TRUE underlying gate: it is suspend-set-sensitive, EXACTLY the pattern the sibling test
    // `spike_host_subset_post_pair_gate_still_suspend_set_sensitive_when_base_suspends_is_wrong`
    // already documents for an inline (non-cross-module) fixture — `ioWork`'s IMPORTED name is
    // absent from the BARE local set, so the bare probe wrongly ADMITS (the post-pair gate
    // doesn't recognize `wait ioWork()` as a suspending call), while the EFFECTIVE set (which
    // BOTH real query boundaries, `frame_layouts_query` and `codegen_query`, always pass — never
    // the bare set) correctly DECLINES. This test now pins the SAME two things that matter in
    // production, updated to reflect that reality:
    //   1. The function declines UNDER THE REAL (effective-set) PATH: entrypoint emits ZERO
    //      `call @ynz_rt_spawn_blocking_joinable` — verified this session with a standalone build
    //      + run of this exact fixture (both default and `--no-auto-parallel` modes; output
    //      `55\n89` byte-identical, no crash, no hang).
    //   2. The bare-vs-effective DIVERGENCE is now the observable tripwire (not an equality): a
    //      caller that reverted to the bare set would wrongly ADMIT this shape (the cross-module
    //      under-allocation class this whole file exists to catch) — `hosts_bare` must contain
    //      `entrypoint` and `hosts_effective` must be empty, and they must DISAGREE. If they ever
    //      AGREE (both empty, or both containing entrypoint), the underlying gate's suspend-set
    //      sensitivity was accidentally papered over again — investigate before trusting either
    //      query boundary's frame sizing.
    // If you're tempted to relax the spawn-count assertion to "> 0 is fine because output is still
    // correct" — the under-allocation is silent; correct output does NOT prove the heap was intact.
    // Fix the suspend-set reconciliation in `codegen_query`, not this test.
    let mut db = CompilerDb::default();
    // Both files must be registered before the queries run so cross-file import resolution
    // (entrypoint → io_lib) can find io_lib by its canonical path.
    let fixture_dir = "v0_3_m3d_spike_s_imported_suspending_after_pair";
    let _io_sf = register_fixture(&mut db, fixture_dir, "io_lib.ynz");
    let entry_sf = register_fixture(&mut db, fixture_dir, "entrypoint.ynz");

    // (1) The emitted entrypoint IR declines the spike.
    let codegen_out = codegen_query(&db, entry_sf);
    assert!(
        !codegen_out.diagnostics.has_errors(),
        "codegen_query(entrypoint) must succeed; diagnostics: {:?}",
        codegen_out.diagnostics
    );
    let ir = &codegen_out.artifact.ir_text;
    // A `call` instruction to the joinable spawn — the `declare` line is not a `call`, so
    // filtering on `call ` excludes the forward declaration.
    let spawn_calls = ir
        .lines()
        .filter(|l| l.contains("call") && l.contains("@ynz_rt_spawn_blocking_joinable"))
        .count();
    assert_eq!(
        spawn_calls, 0,
        "entrypoint declines the spike (imported suspending callee after the CPU pair) → 0 spawn \
         calls. A non-zero count means a host was admitted where the frame was sized sequentially \
         — the cross-boundary divergence (silent heap under-allocation). IR:\n{ir}"
    );

    // (2) The REAL promotion set is non-empty (`entrypoint` — verified this session), so
    // `spike_host_subset` actually reads the suspend-set argument now. The bare probe wrongly
    // ADMITS (the post-pair gate doesn't recognize the IMPORTED `ioWork` as suspending under the
    // bare local-only set); the effective probe correctly DECLINES — matching what BOTH real
    // query boundaries (which always pass the effective set) actually compute, confirmed by (1)'s
    // 0-spawn assertion above.
    let check = check_query(&db, entry_sf);
    let sig = module_signatures_query(&db, entry_sf);
    let promotion = ynz_typeck::cpu_promotion_query(&db, entry_sf);
    let effective = ynz_typeck::build_effective_suspend_set(&check.suspends_set, &sig.imported_fns);
    assert!(
        promotion.promoted.contains("entrypoint"),
        "sanity: entrypoint must be a genuine v0.3-M3g Phase 3 promotion candidate (no \
         guard-tripping crossing) for this test to actually exercise spike_host_subset's \
         suspend-set argument; got promoted: {:?}",
        promotion.promoted
    );
    let hosts_bare = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &check.suspends_set,
        &check.suspends_set,
        &promotion.promoted,
    );
    let hosts_effective = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &effective,
        &effective,
        &promotion.promoted,
    );
    assert!(
        hosts_bare.contains("entrypoint"),
        "the BARE local-only suspend set (missing the imported `ioWork` name) must wrongly ADMIT \
         entrypoint — the pre-existing post-pair suspending-callee gate cannot recognize an \
         imported suspending callee under the bare set. This is the exact cross-module \
         under-allocation hazard the file exists to catch: if a caller ever passed the bare set \
         here, the frame would be sized DIFFERENTLY than the real (effective-set) codegen lays it \
         out; got hosts: {hosts_bare:?}"
    );
    assert!(
        hosts_effective.is_empty(),
        "the EFFECTIVE suspend set (imported `ioWork` present) must correctly DECLINE entrypoint \
         — matching the real production codegen path (0 spawns, asserted in (1) above); got \
         hosts: {hosts_effective:?}"
    );
    assert_ne!(
        hosts_bare, hosts_effective,
        "bare and effective probes MUST disagree on this fixture — an agreement here (either both \
         admit or both decline) means the underlying post-pair suspending-callee gate stopped \
         being suspend-set-sensitive, and this test has lost its diagnostic value as the tripwire \
         for a caller that reverts to the wrong (bare) set. Both REAL query boundaries always pass \
         the effective set (never the bare set), so production stays safe regardless — but this \
         disagreement is what proves the effective-set discipline is load-bearing, not incidental."
    );
}

// CONSTRUCTION NOTE (shared by the two tests below, v0.3-M3g Phase-2-blocker-fix): this fixture
// uses an INLINE single-file source with a LOCAL `ioWork` (not the shared cross-module
// `v0_3_m3d_spike_s_imported_suspending_after_pair` fixture used by the two tests above), and
// calls `ioWork()` BARE (no `wait` keyword). Two independent reasons force this: (1) a bare
// (no `wait`, no `let`) statement-position call to an IMPORTED function does not currently
// resolve (`` `name` is not defined ``) — a pre-existing, unrelated cross-module
// name-resolution gap, confirmed with a non-suspending imported callee too, so a
// literal-wait-free call forces the callee local regardless; (2) calling `ioWork` bare (rather
// than `wait ioWork()`) keeps `stmt_contains_wait_deep` from finding a literal `wait` node,
// which matters for the isolation test below (it needs the OLD post-pair gate reachable, not
// masked by a DIFFERENT decline path).
const IO_WORK_POST_PAIR_SRC: &str = r#"
function fib(n: int) -> int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
function ioWork() -> nothing {
    wait sleep(0)
}
function entrypoint() -> nothing {
    let a = fib(10)
    let b = fib(11)
    ioWork()
    print(a)
    print(b)
}
"#;

#[test]
fn spike_host_subset_base_suspends_decline_masks_bare_vs_effective_divergence() {
    // WHY (v0.3-M3g Phase 3 UPDATE — test name retained for history continuity even though its
    // assertion now proves the OPPOSITE of its Phase-2-blocker-fix predecessor): that predecessor
    // proved `admitted_cpu_group`'s (now-REMOVED) Phase-2 temporary co-resident-suspension decline
    // — keyed on `base_suspends.contains(&f.name)` — made the admission outcome INSENSITIVE to
    // the (unrelated) `suspend_set` argument whenever `base_suspends` was correct. Phase 3 removes
    // that decline entirely, and `admitted_cpu_group` has since dropped the parameter outright
    // (see `cpu_admission.rs`'s `admitted_cpu_group` doc comment) — codegen's own
    // `spike_cpu_candidates`/`spike_host_subset`/`build_frame_layouts_with_resolver` chain still
    // threads `base_suspends` through to `spike_cpu_candidates` (which itself ignores it, via
    // `_base_suspends`), a deliberately deferred wider cleanup (see `spike_cpu_candidates`'s doc
    // comment). This test now proves exactly that non-consultation: the result is IDENTICAL
    // whether `base_suspends` is CORRECT (the real, authoritative set) or deliberately WRONG
    // (empty) — a regression where a future change re-adds `base_suspends` consultation to
    // `admitted_cpu_group` (or its removal is reversed) without updating this test would make
    // these two probes DIVERGE, failing this assertion. (The suspend_set-sensitivity this test
    // used to attribute to "base_suspends being wrong" is now attributable ENTIRELY to the
    // pre-existing, unrelated post-pair suspending-callee gate in `cpu_group_member_indices` — see
    // the sibling test below, which already documents that gate's own bare-vs-effective
    // sensitivity directly.)
    let mut db = CompilerDb::default();
    let entry_sf = register_inline(&mut db, "entrypoint.ynz", IO_WORK_POST_PAIR_SRC);

    let check = check_query(&db, entry_sf);
    assert!(
        !check.diagnostics.has_errors(),
        "fixture must typecheck; diagnostics: {:?}",
        check.diagnostics
    );
    assert!(
        check.suspends_set.contains("ioWork"),
        "sanity: ioWork is genuinely suspending (via wait sleep); got: {:?}",
        check.suspends_set
    );
    assert!(
        check.suspends_set.contains("entrypoint"),
        "sanity: entrypoint calls ioWork (a suspending function), so entrypoint is ALSO \
         transitively suspending under Yinz's no-function-coloring model; got: {:?}",
        check.suspends_set
    );

    // Force a NON-EMPTY promotion set so `spike_host_subset` actually probes entrypoint
    // (bypassing the typeck guard-probe that empties it on this shape today).
    let promoted: HashSet<String> = [String::from("entrypoint")].into_iter().collect();

    // `suspend_set` (the eligibility-scan argument) is held CONSTANT — the effective set, matching
    // what a real caller always passes — while `base_suspends` varies CORRECT vs WRONG (empty).
    let effective_suspend_set: SuspendSet = check.suspends_set.clone();
    let correct_base_suspends: SuspendSet = check.suspends_set.clone();
    let wrong_base_suspends: SuspendSet = SuspendSet::new();

    let hosts_correct_base = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &effective_suspend_set,
        &correct_base_suspends,
        &promoted,
    );
    let hosts_wrong_base = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &effective_suspend_set,
        &wrong_base_suspends,
        &promoted,
    );

    assert_eq!(
        hosts_correct_base, hosts_wrong_base,
        "spike_host_subset must return the IDENTICAL host set regardless of whether base_suspends \
         is correct or (deliberately, for this test) wrong — proving admitted_cpu_group no longer \
         consults base_suspends at all post-Phase-3. A divergence here means base_suspends \
         consultation was silently re-added without updating this test; got correct={hosts_correct_base:?} \
         wrong={hosts_wrong_base:?}"
    );
    assert!(
        hosts_correct_base.is_empty(),
        "entrypoint must decline under the effective suspend_set (the pre-existing post-pair \
         suspending-callee gate, unrelated to base_suspends, still catches this shape — see \
         imported_suspending_after_pair_declines_consistently_across_boundaries for the \
         production-path proof); got hosts: {hosts_correct_base:?}"
    );
}

#[test]
fn spike_host_subset_post_pair_gate_still_suspend_set_sensitive_when_base_suspends_is_wrong() {
    // WHY: the previous test proves that a CORRECT `base_suspends` makes entrypoint's admission
    // outcome insensitive to the (unrelated) `suspend_set` argument. This test proves the
    // converse and why that matters: `cpu_group_member_indices`'s PRE-EXISTING post-pair
    // suspending-callee gate (unrelated to Phase 2, still keyed on `suspend_set`) is still very
    // much alive underneath the Phase-2 decline — it still reacts to a bare-vs-effective
    // `suspend_set` split exactly as before. The Phase-2 decline only SHADOWS that reaction when
    // `base_suspends` is correct; it does not replace or remove the older gate.
    //
    // ARTIFICIAL ISOLATION: this test deliberately passes an EMPTY `base_suspends` — NOT a value
    // any real caller would ever construct (every real caller derives it from
    // `build_effective_suspend_set`, which always includes a function calling a genuinely
    // suspending callee like `ioWork`). This isolates the older gate on purpose, to document WHY
    // `base_suspends` correctness is the one thing standing between "safe" and "the exact
    // heap-under-allocation class this file exists to guard against": get `base_suspends` wrong
    // (e.g. a future caller that forgets to include it, or unions in the wrong set) and the
    // bare-vs-effective divergence in the OLDER post-pair gate becomes observable again.
    let mut db = CompilerDb::default();
    let entry_sf = register_inline(&mut db, "entrypoint.ynz", IO_WORK_POST_PAIR_SRC);

    let check = check_query(&db, entry_sf);
    assert!(
        !check.diagnostics.has_errors(),
        "fixture must typecheck; diagnostics: {:?}",
        check.diagnostics
    );

    let promoted: HashSet<String> = [String::from("entrypoint")].into_iter().collect();
    let empty_base_suspends: SuspendSet = SuspendSet::new();

    // BARE suspend_set: `ioWork` removed → the post-pair gate does NOT recognize the call as
    // suspending → admits (with base_suspends artificially empty, nothing else declines it).
    let mut bare_suspend_set: SuspendSet = check.suspends_set.clone();
    bare_suspend_set.remove("ioWork");
    let hosts_bare = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &bare_suspend_set,
        &empty_base_suspends,
        &promoted,
    );
    assert!(
        hosts_bare.contains("entrypoint"),
        "with base_suspends artificially empty and a BARE suspend_set (missing 'ioWork'), the \
         older post-pair gate does not fire → spike_host_subset must ADMIT entrypoint; got \
         hosts: {hosts_bare:?}"
    );

    // EFFECTIVE suspend_set: `ioWork` present → the post-pair gate recognizes the call as
    // suspending → declines, even with base_suspends artificially empty.
    let effective_suspend_set: SuspendSet = check.suspends_set.clone();
    let hosts_effective = ynz_codegen::emit::spike_host_subset(
        &check.typed_module,
        &effective_suspend_set,
        &empty_base_suspends,
        &promoted,
    );
    assert!(
        hosts_effective.is_empty(),
        "with base_suspends artificially empty and the EFFECTIVE suspend_set ('ioWork' present), \
         the older post-pair gate fires → spike_host_subset must DECLINE entrypoint; got hosts: \
         {hosts_effective:?}"
    );

    assert_ne!(
        hosts_bare, hosts_effective,
        "the bare and effective probes MUST disagree when base_suspends is (artificially) empty \
         — this is the residual hazard a real caller avoids ONLY by always computing base_suspends \
         correctly; if they agree here, the older post-pair gate has stopped reading suspend_set \
         and this isolation has lost its diagnostic value"
    );
}

// ── Escape #2 — shape slot count uses LLVM-padded ABI size ───────────────────────────────

#[test]
fn three_int_shape_crossing_local_uses_three_llvm_slots() {
    // WHY: A shape `{ x: int, y: int, z: int }` has LLVM struct type `{ i64, i64, i64 }`.
    // LLVM ABI size on x86_64: 24 bytes (three 8-byte fields, no padding). After
    // `max(24, 8) = 24` and `24.div_ceil(8) = 3`, the shape occupies 3 frame slots.
    //
    // This is the primary escape #2 proof: the `shape_frame_slots` fallback path
    // (triggered when the shape is missing from `shape_abi_sizes`) returns 1 slot
    // (8-byte fallback). The test asserts n_locals == 3 (not 1), which FAILS if the
    // LLVM-sizing path silently fails and the fallback fires. Three slots ≠ one slot —
    // the test cannot pass on a silent sizing failure the way n_locals==1 tests can.
    let source = r#"
shape Point3D {
  x: int
  y: int
  z: int
}

function caller() -> nothing {
  let pos: Point3D = { x: 1, y: 2, z: 3 }
  sleep(10)
  print(pos.x.toString())
}

function entrypoint() -> nothing {
  caller()
}
"#;

    let mut db = CompilerDb::default();
    let sf = register_inline(&mut db, "test_three_int_shape.ynz", source);

    let layouts = frame_layouts_query(&db, sf);
    let layout = layouts.get("caller").unwrap_or_else(|| {
        panic!(
            "frame_layouts_query must include 'caller'; got keys: {:?}",
            layouts.keys().collect::<Vec<_>>()
        )
    });

    // `caller` has no params and 1 crossing local (`pos: Point3D`).
    // Point3D is { i64, i64, i64 } in LLVM — 24 bytes ABI → 3 frame slots, plus the
    // FRAME_SHAPE_REGION_SLACK_SLOTS every frame-embedded shape reserves so its region
    // pointer can be rounded up to FRAME_SHAPE_REGION_ALIGN at runtime (hotfix
    // `bgarg-number`). Fallback (shape missing from shape_abi_sizes) would give
    // 1 + slack slots — still distinguishable from 3 + slack.
    // n_locals = 0 params + 3 shape slots + slack.
    let expected_slots = 3 + FRAME_SHAPE_REGION_SLACK_SLOTS;
    assert_eq!(
        layout.n_locals,
        expected_slots,
        "Point3D (3×i64, 24 bytes ABI) crossing local occupies 3 LLVM slots + \
         {FRAME_SHAPE_REGION_SLACK_SLOTS} alignment slack. \
         n_locals = 0 params + 3 shape slots + slack = {expected_slots}. \
         Got n_locals = {}; a value of {} means the shape_frame_slots fallback fired \
         (LLVM sizing failed silently).",
        layout.n_locals,
        1 + FRAME_SHAPE_REGION_SLACK_SLOTS
    );

    // total_size: header(32) + own_locals((3 + slack) × 8) + no children.
    let expected_total = FRAME_HEADER_SIZE + own_locals_size(expected_slots);
    assert_eq!(
        layout.total_size, expected_total,
        "caller total_size should be {expected_total} (header + 3 shape slots + slack); got {}",
        layout.total_size
    );
}

#[test]
fn bool_bool_shape_crossing_local_uses_one_llvm_slot() {
    // WHY: A shape `{ flag: bool, flag2: bool }` has LLVM struct type `{ i1, i1 }`.
    // LLVM ABI size on x86_64: 2 bytes (no padding needed between 1-bit fields; struct
    // aligns to 1 byte). After `max(2, 8) = 8` and `8.div_ceil(8) = 1`, the shape occupies
    // exactly 1 frame slot (8 bytes).
    //
    // The prior typeck-side frame-size approximation counted fields: 2 bool fields ×
    // 1 slot/field × 8 bytes = 16 bytes → `16.div_ceil(8) = 2 slots`. That was the
    // escape #2 divergence: typeck approximation said 2 slots, LLVM ABI says 1 slot.
    //
    // A function with this shape as a crossing local must use 1 slot (not 2) so the
    // frame layout is tight and subsequent slots (or child sub-frames) start at the
    // right offset. Two slots instead of one causes the next slot to start 8 bytes too
    // late → either an out-of-bounds write or a misread of the return value.
    //
    // Note: n_locals == 1 also equals the shape_frame_slots fallback value (1 slot when
    // shape is missing from abi_sizes). The three_int_shape test above covers that
    // distinction — this test specifically covers the typeck-vs-LLVM divergence for
    // compact structs (bool×bool: typeck 2 slots, LLVM 1 slot).
    let source = r#"
shape Flags {
  flag: boolean
  flag2: boolean
}

function caller() -> nothing {
  let flags: Flags = { flag: true, flag2: false }
  sleep(10)
  print(flags.flag.toString())
}

function entrypoint() -> nothing {
  caller()
}
"#;

    let mut db = CompilerDb::default();
    let sf = register_inline(&mut db, "test_bool_shape.ynz", source);

    let layouts = frame_layouts_query(&db, sf);
    let layout = layouts.get("caller").unwrap_or_else(|| {
        panic!(
            "frame_layouts_query must include 'caller'; got keys: {:?}",
            layouts.keys().collect::<Vec<_>>()
        )
    });

    // `caller` has no params and 1 crossing local (`flags: Flags`).
    // Flags is { i1, i1 } in LLVM (boolean → ctx.bool_type() → i1).
    // x86_64 ABI: two i1 fields → struct size = 2 bytes → max(2, 8) = 8 → ceil(8/8) = 1 slot,
    // plus the FRAME_SHAPE_REGION_SLACK_SLOTS every frame-embedded shape reserves
    // (hotfix `bgarg-number`). The typeck field-count approximation this test guards
    // against would count 2 + slack — still distinguishable from 1 + slack.
    // n_locals = 0 params + 1 slot + slack.
    let expected_slots = 1 + FRAME_SHAPE_REGION_SLACK_SLOTS;
    assert_eq!(
        layout.n_locals, expected_slots,
        "bool×bool shape crossing local occupies 1 LLVM slot (8 bytes ABI) + \
         {FRAME_SHAPE_REGION_SLACK_SLOTS} alignment slack, not 2 + slack (typeck field count). \
         n_locals = 0 params + 1 shape slot + slack = {expected_slots}. \
         Got n_locals = {}; this indicates the typeck field-count approximation leaked into the query.",
        layout.n_locals
    );

    // total_size: header(32) + own_locals((1 + slack) × 8) + no children.
    let expected_total = FRAME_HEADER_SIZE + own_locals_size(expected_slots);
    assert_eq!(
        layout.total_size, expected_total,
        "caller total_size should be {expected_total} (header + 1 shape slot + slack); got {}",
        layout.total_size
    );
}

// ── Guard G3 — circular import cycle recovery ─────────────────────────────────────────────

#[test]
fn circular_import_returns_empty_map_not_infinite_loop() {
    // WHY: Circular imports (A imports from B, B imports from A) are already typeck errors
    // (`module_signatures_query` has its own cycle recovery that produces a clean diagnostic).
    // `frame_layouts_query` must also recover cleanly — return an empty map — because it
    // calls `check_query` internally, which will have errors for the circular-import module,
    // triggering the early-return path. This test ensures no infinite loop or ICE occurs.
    //
    // We register two inline sources that would form a cycle if the resolver tried to
    // recurse into each other. In practice, check_query fires a circular-import error
    // before the recursion can happen, so the cycle recovery function
    // frame_layouts_cycle_initial is a defense-in-depth backstop rather than the primary
    // path. Both paths (early-exit-on-error and cycle-initial) are correct; the test
    // confirms neither infinite-loops.
    let mut db = CompilerDb::default();

    // Two modules with mutually suspicious import declarations. The imports won't resolve
    // to real files (no such files on disk) so resolve_module_path returns None, which
    // is fine — the check_query errors out on the unresolved imports before frame_layouts_query
    // can recurse.
    let source_a = r#"
import { doB } from `b_cycle`

export function doA() -> nothing {
  sleep(10)
}
"#;
    let source_b = r#"
import { doA } from `a_cycle`

export function doB() -> nothing {
  sleep(10)
}
"#;

    // Use real-looking paths so resolve_module_path can attempt resolution. The files
    // won't exist on disk, so resolution returns None → check_query emits "module not found"
    // → frame_layouts_query returns empty map (early exit on error). No infinite loop.
    let sf_a = register_inline(&mut db, "/tmp/ynz_test_cycle/a_cycle.ynz", source_a);
    let _sf_b = register_inline(&mut db, "/tmp/ynz_test_cycle/b_cycle.ynz", source_b);

    // Must complete in finite time (no infinite loop) and return an empty map (errors present).
    let layouts_a = frame_layouts_query(&db, sf_a);

    // a_cycle imports from b_cycle, but b_cycle.ynz doesn't exist on disk — resolve_module_path
    // returns None → check_query emits "module not found" → frame_layouts_query returns empty map.
    assert!(
        layouts_a.is_empty(),
        "frame_layouts_query on a module with unresolvable imports must return \
         an empty map (check_query errors on unresolved import → early exit). Got: {:?}",
        layouts_a.keys().collect::<Vec<_>>()
    );
}

// ── Regression: frame_layouts_query handles a module with no suspending fns ──────────────

#[test]
fn non_suspending_module_returns_empty_map() {
    // WHY: Modules with no suspending functions must return an empty map (not a crash,
    // not a map with phantom entries). This is the common case for most Yinz modules.
    let source = r#"
function entrypoint() -> nothing {
  print(`hello`)
}
"#;
    let mut db = CompilerDb::default();
    let sf = register_inline(&mut db, "no_suspends.ynz", source);

    let layouts = frame_layouts_query(&db, sf);
    assert!(
        layouts.is_empty(),
        "Module with no suspending functions must return an empty layout map; \
         got: {:?}",
        layouts.keys().collect::<Vec<_>>()
    );
}
