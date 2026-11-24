// WHY: the `let→const` promotion hint must fire only when a binding is provably never
// mutated. A binding is mutated when it appears as an assignment target, as the receiver of a
// mutating method call, or as an argument to a `lend`/`give` parameter (also behind a field or
// index chain, inside a `wait`, inside an interpolated string, inside a nested call, or in an
// `else =>` arm). Passing a binding to a `share` parameter (or to any builtin free-fn or scalar
// primitive intrinsic method, which are all read-only by contract) is not a mutation. When the
// callee cannot be resolved the walker marks the argument mutated: a missed hint is preferred
// over a wrong "effectively const" claim.
//
// One unit: `let_to_const_promotion_hints` — does this binding get the hint. Each case is one
// mutation path (or one over-suppression control) of the maybe-mutated walker.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{let_to_const_promotion_hints, PromotionKind};

/// What the query must return for a program, counted over `LetToConst` hints.
#[derive(Debug)]
enum LetToConstHints {
    /// At least one binding gets the hint.
    Fires,
    /// No binding gets the hint.
    Suppressed,
    /// Exactly this many bindings get the hint.
    Exactly(usize),
    /// At most this many bindings get the hint.
    AtMost(usize),
}

fn let_to_const_hint_count(src: &str) -> usize {
    let path = format!("/tmp/ynz_maybe_mutated_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    let_to_const_promotion_hints(&db, sf)
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .count()
}

fn assert_let_to_const_hints(src: &str, expected: LetToConstHints) {
    let count = let_to_const_hint_count(src);
    let matches_expected = match expected {
        LetToConstHints::Fires => count >= 1,
        LetToConstHints::Suppressed => count == 0,
        LetToConstHints::Exactly(wanted) => count == wanted,
        LetToConstHints::AtMost(limit) => count <= limit,
    };
    assert!(
        matches_expected,
        "expected {expected:?} let→const hints, got {count} for:\n{src}"
    );
}

macro_rules! let_to_const_cases {
    ($($case:ident: $src:expr => $expected:expr, $bug:literal;)+) => {
        $(
            // bug: the let→const hint fires on a binding that is mutated (or is suppressed on one that is not), so the IDE suggests a change the compiler then refuses | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_let_to_const_hints($src, $expected);
            }
        )+
    };
}

let_to_const_cases! {
    share_param_call_does_not_suppress_hint:
        r#"
function report(share val: Int) -> Nothing { }
function logger(share msg: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let count = 5
  report(count)
  logger(count)
}
"#
        => LetToConstHints::Fires,
        "a binding passed only to `share` parameters is treated as mutated (Bug 2.9: nearly every real program passes `let`s to reporting functions)";

    lend_param_call_suppresses_hint:
        r#"
function consume(lend x: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let count = 0
  consume(count)
}
"#
        => LetToConstHints::Suppressed,
        "a binding passed to a `lend` parameter keeps its hint although the callee may mutate it";

    give_param_call_suppresses_hint:
        r#"
function take(give x: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let val = 7
  take(val)
}
"#
        => LetToConstHints::Suppressed,
        "a binding handed to a `give` parameter (consumed) still gets the const hint";

    unresolvable_callee_conservatively_suppresses_hint:
        r#"
function entrypoint() -> Nothing {
  let x = 10
  unknownFn(x)
}
"#
        => LetToConstHints::Suppressed,
        "an argument of an unresolvable callee is claimed const although its ownership is unknown";

    share_param_plus_reassign_still_suppresses_hint:
        r#"
function display(share val: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let x = 5
  display(x)
  x = 9
}
"#
        => LetToConstHints::Suppressed,
        "callee resolution for a `share` pass hides the direct reassignment of the same binding";

    lend_call_nested_in_a_call_argument_is_tracked:
        r#"
function identity_lend(lend x: Int) -> Int { 0 }
function make_thing(share a: Int, share b: Int) -> Int { 0 }

function entrypoint() -> Nothing {
  let a = 10
  let b = 20
  let _result = make_thing(a, identity_lend(b))
}
"#
        => LetToConstHints::Exactly(2),
        "a `lend` call nested inside another call's argument is missed, so `b` keeps its hint (`a` and `_result` get one)";

    const_binding_gets_no_hint:
        r#"
function display(share val: Int) -> Nothing { }

function entrypoint() -> Nothing {
  const x = 99
  display(x)
}
"#
        => LetToConstHints::Suppressed,
        "the hint fires on a binding that is already `const`";

    print_call_does_not_suppress_hint:
        r#"
function entrypoint() -> Nothing {
  let count = 5
  print(count)
}
"#
        => LetToConstHints::Fires,
        "the unknown-callee fallback also applies to the read-only builtin `print`, so the commonest beginner program never shows the hint";

    intrinsic_method_receiver_does_not_suppress_hint:
        r#"
function entrypoint() -> Nothing {
  let score = 5
  const s = score.toString()
}
"#
        => LetToConstHints::Exactly(1),
        "a read-only intrinsic method (`.toString()`) counts as a mutation of its receiver (`s` is `const` so only `score` can fire)";

    mutation_in_else_arm_suppresses_hint:
        r#"
function entrypoint() -> Nothing {
  let count = 0
  if (count) {
    1 => {}
    else => {
      count = 99
    }
  }
}
"#
        => LetToConstHints::Suppressed,
        "the `else =>` arm of a match is skipped by the walker, so a reassignment there is invisible (Bug 2.6)";

    no_mutation_in_else_arm_still_fires:
        r#"
function entrypoint() -> Nothing {
  let count = 0
  if (count) {
    1 => {}
    else => {
      let other = 5
    }
  }
}
"#
        => LetToConstHints::Fires,
        "visiting the `else =>` arm over-suppresses the hint when the arm does not mutate the binding";

    nested_field_assign_marks_the_root_binding:
        r#"
function entrypoint() -> Nothing {
  let player = { name: "Carey", address: { street: "Federal St", city: "Pittsburgh" } }
  player.address.street = "Mon Wharf"
}
"#
        => LetToConstHints::Suppressed,
        "`player.address.street = ..` records the intermediate `address` instead of the root `player` (Bug 2.7)";

    nested_index_assign_marks_the_root_binding:
        r#"
function entrypoint() -> Nothing {
  let arr: Array<Array<Int>> = [[1, 2], [3, 4]]
  let i = 0
  let j = 1
  arr[i][j] = 99
}
"#
        => LetToConstHints::Exactly(2),
        "`arr[i][j] = v` leaves `arr` untracked, so it keeps its hint (only `i` and `j` may fire) (Bug 2.7)";

    method_call_through_a_field_chain_marks_the_root_binding:
        r#"
shape Address { street: String }
shape Player { name: String, address: Address, health: Int }
function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
function entrypoint() -> Nothing {
  let addr = { street: "Federal St" }
  let player = { name: "Carey", address: addr, health: 80 }
  player.address.heal(5)
}
"#
        => LetToConstHints::Exactly(1),
        "`player.address.heal(5)` leaves the root `player` unrecorded, so both lets fire (only `addr`, never mutated, may)";

    wait_wrapped_lend_call_suppresses_hint:
        r#"
function slowFill(lend buf: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let buf = 0
  wait slowFill(buf)
}
"#
        => LetToConstHints::Suppressed,
        "the walker does not recurse into `wait`, so a lend call behind it is invisible";

    wait_wrapped_share_call_does_not_suppress_hint:
        r#"
function readOnly(share x: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let count = 5
  wait readOnly(count)
}
"#
        => LetToConstHints::Fires,
        "everything wrapped in `wait` is suppressed, including read-only `share` calls";

    interpolated_string_lend_call_suppresses_hint:
        r#"
function bump(lend x: Int) -> Int { 0 }

function entrypoint() -> Nothing {
  let buf = 0
  let msg = `count is ${bump(buf)}`
}
"#
        => LetToConstHints::AtMost(1),
        "the walker skips `${..}` parts of an interpolated string, so a lend call there leaves `buf` with a hint (only `msg` may fire)";
}
