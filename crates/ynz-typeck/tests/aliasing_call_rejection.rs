// WHY: v0.3-M7 Phase 2 (FRAGO 002, Patrick-directed 2026-07-16) — a call passing the
// same value (or overlapping pieces of one value) into two parameter positions where at
// least one position can modify it violates the ownership contract (`lend` = exclusive
// mutable access) and MUST be a compile error (Golden Rule 2). Typeck previously
// accepted these calls while codegen claimed LLVM `noalias` on both parameters — a false
// claim the optimizer exploited into a silent miscompile (RED fixture
// `v0_3_m7_p1_share_lend_alias.ynz`). These cases lock the rejection and its scope
// boundaries (read-read overlap stays legal; scalars are exempt).

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

const BOX_SHAPE: &str = "shape Box {\n  value: Int\n}\n";

// One case per input class the aliasing rule decides differently; `assert_aliasing_rejected`
// cases carry the key phrase of the rejection, `assert_clean` cases must be accepted.
macro_rules! aliasing_cases {
    ($($case:ident: $source:expr => $verdict:ident $(($phrase:expr))?;)+) => {
        $(
            // bug: typeck accepts a call that passes one value (or a part of it) into two parameter positions where one can write (codegen's `noalias` then miscompiles), or refuses a legal read-read or scalar overlap | tier: High
            #[test]
            fn $case() {
                $verdict(&$source $(, $phrase)?);
            }
        )+
    };
}

aliasing_cases! {
    // the exact RED-fixture shape: `relay(h, h)` against `(share, lend)`
    same_value_as_share_and_lend_is_rejected:
        format!(
            "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> Nothing {{
  dst.value = 5
  dst.value = src.value
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  relay(h, h)
  print(h.value.toString())
}}"
        ) => assert_aliasing_rejected("passed to `relay` twice in the same call");

    // FRAGO 002 names "two aliasing `lend`s" explicitly: both sides can write
    same_value_as_two_lends_is_rejected:
        format!(
            "{BOX_SHAPE}
function swap(lend a: Box, lend b: Box) -> Nothing {{
  let t: Int = a.value
  a.value = b.value
  b.value = t
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  swap(h, h)
  print(h.value.toString())
}}"
        ) => assert_aliasing_rejected("passed to `swap` twice in the same call");

    // read-read overlap is harmless: neither position writes
    same_value_as_two_shares_is_accepted:
        format!(
            "{BOX_SHAPE}
function larger(share a: Box, share b: Box) -> Int {{
  if (a.value > b.value) {{ return a.value }}
  return b.value
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  print(larger(h, h).toString())
}}"
        ) => assert_clean;

    // a bare parameter the body mutates is an INFERRED `lend`: the rejection must consult the
    // effective ownership, not just the declared modifier
    same_value_into_bare_mutating_param_is_rejected:
        format!(
            "{BOX_SHAPE}
function bumpBoth(a: Box, share b: Box) -> Nothing {{
  a.value = a.value + b.value
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  bumpBoth(h, h)
  print(h.value.toString())
}}"
        ) => assert_aliasing_rejected("passed to `bumpBoth` twice in the same call");

    // BOTH parameters bare: each position's write-capability comes only from effective
    // ownership (`Writes`), never a declared `lend`
    same_value_into_two_bare_mutating_params_is_rejected:
        format!(
            "{BOX_SHAPE}
function bumpBoth(a: Box, b: Box) -> Nothing {{
  a.value = a.value + 1
  b.value = b.value + 1
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  bumpBoth(h, h)
  print(h.value.toString())
}}"
        ) => assert_aliasing_rejected("passed to `bumpBoth` twice in the same call");

    // `player.pet` is part of `player`: a write through the whole reaches the part
    overlapping_field_path_with_lend_is_rejected:
        "shape Pet {
  age: Int
}
shape Player {
  score: Int
  pet: Pet
}
function groom(lend whole: Player, share part: Pet) -> Nothing {
  whole.score = whole.score + part.age
}
function entrypoint() -> Nothing {
  let p: Player = { score: 0, pet: { age: 3 } }
  groom(p, p.pet)
  print(p.score.toString())
}" => assert_aliasing_rejected("is part of");

    // the rejection keys on the VALUE overlapping, never on the modifier pair alone
    distinct_values_share_and_lend_is_accepted:
        format!(
            "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> Nothing {{
  dst.value = src.value
}}
function entrypoint() -> Nothing {{
  let a: Box = {{ value: 1 }}
  let b: Box = {{ value: 2 }}
  relay(a, b)
  print(b.value.toString())
}}"
        ) => assert_clean;

    // `h.relay(h)` is parser-level sugar for `relay(h, h)`: both call forms reject identically
    ufcs_receiver_aliasing_arg_is_rejected:
        format!(
            "{BOX_SHAPE}
function relay(share src: Box, lend dst: Box) -> Nothing {{
  dst.value = src.value
}}
function entrypoint() -> Nothing {{
  let h: Box = {{ value: 1 }}
  h.relay(h)
  print(h.value.toString())
}}"
        ) => assert_aliasing_rejected("passed to `relay` twice in the same call");

    // Int/float/bool pass by value: two copies never touch the same storage
    same_scalar_value_twice_is_accepted:
        "function add(a: Int, b: Int) -> Int {
  return a + b
}
function entrypoint() -> Nothing {
  let n: Int = 4
  print(add(n, n).toString())
}" => assert_clean;
}
