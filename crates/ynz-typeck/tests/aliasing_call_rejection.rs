// WHY: v0.3-M7 Phase 2 (FRAGO 002, Patrick-directed 2026-07-16) — a call passing the
// same value (or overlapping pieces of one value) into two parameter positions where at
// least one position can modify it violates the ownership contract (`lend` = exclusive
// mutable access) and MUST be a compile error (Golden Rule 5). Typeck previously
// accepted these calls while codegen claimed LLVM `noalias` on both parameters — a false
// claim the optimizer exploited into a silent miscompile (RED fixture
// `v0_3_m7_p1_share_lend_alias.ynz`). These tests lock the rejection, its scope
// boundaries (read-read overlap stays legal; scalars are exempt), and its teaching shape.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

const FILE: &str = "test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn error_messages(output: &CheckOutput) -> Vec<String> {
    output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .map(|d| format!("{d:?}"))
        .collect()
}

fn assert_clean(source: &str) {
    let output = run(source);
    let errors = error_messages(&output);
    assert!(
        errors.is_empty(),
        "Expected 0 errors, got {}: {:#?}",
        errors.len(),
        errors
    );
}

fn assert_aliasing_rejected(source: &str, expected_phrase: &str) {
    let output = run(source);
    let errors = error_messages(&output);
    assert!(
        !errors.is_empty(),
        "Expected the aliasing call to be rejected, but typeck accepted it"
    );
    assert!(
        errors.iter().any(|e| e.contains(expected_phrase)),
        "Expected an error containing {expected_phrase:?}; got: {errors:#?}"
    );
}

const BOX_SHAPE: &str = "shape Box {\n  value: int\n}\n";

#[test]
fn same_value_as_share_and_lend_is_rejected() {
    // WHY: the exact RED-fixture shape — `relay(h, h)` against `(share, lend)`.
    let src = format!(
        "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> nothing {{
  dst.value = 5
  dst.value = src.value
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  relay(h, h)
  print(h.value.toString())
}}"
    );
    assert_aliasing_rejected(&src, "passed to `relay` twice in the same call");
}

#[test]
fn same_value_as_two_lends_is_rejected() {
    // WHY: FRAGO 002 names "two aliasing `lend`s" explicitly — both sides can write,
    // so the exclusivity each position needs is doubly violated.
    let src = format!(
        "{BOX_SHAPE}
function swap(lend a: Box, lend b: Box) -> nothing {{
  let t: int = a.value
  a.value = b.value
  b.value = t
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  swap(h, h)
  print(h.value.toString())
}}"
    );
    assert_aliasing_rejected(&src, "passed to `swap` twice in the same call");
}

#[test]
fn same_value_as_two_shares_is_accepted() {
    // WHY: read-read overlap is harmless — neither position writes, and LLVM's
    // parameter attributes explicitly permit shared reads. Rejecting it would ban a
    // legitimate pattern for no soundness gain.
    let src = format!(
        "{BOX_SHAPE}
function larger(share a: Box, share b: Box) -> int {{
  if (a.value > b.value) {{ return a.value }}
  return b.value
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  print(larger(h, h).toString())
}}"
    );
    assert_clean(&src);
}

#[test]
fn same_value_into_bare_mutating_param_is_rejected() {
    // WHY: a bare parameter the body mutates is an INFERRED `lend` (effective
    // ownership `Writes`) — the rejection must consult the effective answer, not just
    // the declared modifier, or the inferred-lend case slips through unsound.
    let src = format!(
        "{BOX_SHAPE}
function bumpBoth(a: Box, share b: Box) -> nothing {{
  a.value = a.value + b.value
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  bumpBoth(h, h)
  print(h.value.toString())
}}"
    );
    assert_aliasing_rejected(&src, "passed to `bumpBoth` twice in the same call");
}

#[test]
fn same_value_into_two_bare_mutating_params_is_rejected() {
    // WHY: BOTH parameters are bare — each position's write-capability comes only
    // from effective ownership (`Writes`), never a declared `lend`. If the rejection
    // keyed on declared modifiers at any point, this doubly-inferred pair would slip
    // through while codegen (correctly consulting the same effective answer) claims
    // `noalias` on both positions — the exact silent-miscompile shape FRAGO 002 locks.
    let src = format!(
        "{BOX_SHAPE}
function bumpBoth(a: Box, b: Box) -> nothing {{
  a.value = a.value + 1
  b.value = b.value + 1
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  bumpBoth(h, h)
  print(h.value.toString())
}}"
    );
    assert_aliasing_rejected(&src, "passed to `bumpBoth` twice in the same call");
}

#[test]
fn overlapping_field_path_with_lend_is_rejected() {
    // WHY: `player.pet` is part of `player` — a write through the whole reaches the
    // part, so a prefix-overlapping pair is the same conflict as an identical pair.
    let src = "shape Pet {
  age: int
}
shape Player {
  score: int
  pet: Pet
}
function groom(lend whole: Player, share part: Pet) -> nothing {
  whole.score = whole.score + part.age
}
function entrypoint() -> nothing {
  let p: Player = { score: 0, pet: { age: 3 } }
  groom(p, p.pet)
  print(p.score.toString())
}";
    assert_aliasing_rejected(src, "is part of");
}

#[test]
fn distinct_values_share_and_lend_is_accepted() {
    // WHY: the rejection must key on the VALUE overlapping, never on the modifier
    // pair alone — two independent values into (share, lend) is the normal case.
    let src = format!(
        "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> nothing {{
  dst.value = src.value
}}
function entrypoint() -> nothing {{
  let a: Box = {{ value: 1 }}
  let b: Box = {{ value: 2 }}
  relay(a, b)
  print(b.value.toString())
}}"
    );
    assert_clean(&src);
}

#[test]
fn ufcs_receiver_aliasing_arg_is_rejected() {
    // WHY: `h.relay(h)` is parser-level sugar for `relay(h, h)` (UFCS) — both call
    // forms must reject identically or the sugar becomes a soundness loophole.
    let src = format!(
        "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> nothing {{
  dst.value = src.value
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  h.relay(h)
  print(h.value.toString())
}}"
    );
    assert_aliasing_rejected(&src, "passed to `relay` twice in the same call");
}

#[test]
fn same_scalar_value_twice_is_accepted() {
    // WHY: int/float/bool pass by value — two copies can never touch the same
    // storage, so the aliasing rule must not fire on scalars.
    let src = "function add(a: int, b: int) -> int {
  return a + b
}
function entrypoint() -> nothing {
  let n: int = 4
  print(add(n, n).toString())
}";
    assert_clean(src);
}

#[test]
fn rejection_diagnostic_carries_teaching_shape() {
    // WHY: Golden Rule 11 — the error must name WHAT (both positions and their
    // meanings), WHAT-INSTEAD (a copyable fix: `.copy()`), and a non-circular WHY,
    // with no compiler jargon.
    let src = format!(
        "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> nothing {{
  dst.value = src.value
}}
function entrypoint() -> nothing {{
  let h: Box = {{ value: 1 }}
  relay(h, h)
}}"
    );
    let output = run(&src);
    let errors = error_messages(&output);
    let joined = errors.join("\n");
    for phrase in [
        "`share` (a read-only view)",
        "`lend` (the function modifies it)",
        ".copy()",
        "only way that value is reached",
    ] {
        assert!(
            joined.contains(phrase),
            "teaching diagnostic must contain {phrase:?}; got:\n{joined}"
        );
    }
}
