/// Property tests for decimal128 arithmetic.
///
/// WHY: property tests catch algorithmic bugs that unit tests miss by verifying
/// structural invariants across many inputs.  A bug in commutativity means the
/// Pratt climber's left-to-right vs right-to-left evaluation produces different
/// results; a round-trip failure means literals can't survive parse→format→parse.
use proptest::prelude::*;
use ynz_numerics::format as d128_fmt;
use ynz_numerics::{add, compare, mul, neg, parse, sub};

fn parse_or_zero(s: &str) -> u128 {
    parse(s).unwrap_or(ynz_numerics::POS_ZERO)
}

fn representative() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "0",
        "1",
        "-1",
        "2",
        "-2",
        "0.1",
        "0.5",
        "0.9",
        "0.01",
        "10",
        "100",
        "-10",
        "1.5",
        "-1.5",
        "2.25",
        "0.3333333333333333333333333333333333",
        "1e5",
        "1e-5",
        "1.23e10",
        "9999999999999999999999999999999999",
        "-9999999999999999999999999999999999",
    ])
    .prop_map(|s| s.to_string())
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 500,
        ..Default::default()
    })]

    // bug: decimal128 add gives a different result when its operands are swapped (exponent alignment or sign handling has a directional bug) | tier: High
    #[test]
    fn commutativity_add(a in representative(), b in representative()) {
        // WHY: a + b == b + a.  A violation means the exponent alignment or sign
        // handling has a directional bug.
        let ab = add(parse_or_zero(&a), parse_or_zero(&b));
        let ba = add(parse_or_zero(&b), parse_or_zero(&a));
        prop_assert_eq!(d128_fmt(ab), d128_fmt(ba),
            "commutativity violated: {} + {}", a, b);
    }

    // bug: decimal128 add of zero changes the other operand's value | tier: High
    #[test]
    fn identity_add(a in representative()) {
        // WHY: a + 0 == a.  If this fails, the zero-handling special case is wrong.
        let a_bits = parse_or_zero(&a);
        let result = add(a_bits, ynz_numerics::POS_ZERO);
        let same = match (parse(&d128_fmt(a_bits)), parse(&d128_fmt(result))) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
        prop_assert!(same, "a + 0 != a for a={}: got {}", a, d128_fmt(result));
    }

    // bug: decimal128 sub of a value from itself is not zero | tier: High
    #[test]
    fn a_minus_a_is_zero(a in representative()) {
        // WHY: a - a must equal 0.
        let a_bits = parse_or_zero(&a);
        let result = sub(a_bits, a_bits);
        let v = ynz_numerics::decode(result);
        prop_assert!(v.is_zero() || v.coefficient == 0,
            "a - a != 0 for a={}: got {}", a, d128_fmt(result));
    }

    // bug: decimal128 sub gives a result that is not the negation of the swapped subtraction | tier: High
    #[test]
    fn sub_antisymmetry(a in representative(), b in representative()) {
        // WHY: a - b == -(b - a) numerically.  +0 and -0 are equal per IEEE 754,
        // but format() produces different strings ("0" vs "-0") — use compare().
        let ab = sub(parse_or_zero(&a), parse_or_zero(&b));
        let ba = neg(sub(parse_or_zero(&b), parse_or_zero(&a)));
        let equal = d128_fmt(ab) == d128_fmt(ba) || compare(ab, ba) == 0;
        prop_assert!(equal, "a-b != -(b-a) for {}, {}: {} vs {}", a, b, d128_fmt(ab), d128_fmt(ba));
    }

    // bug: decimal128 mul gives a different result when its operands are swapped (asymmetric operand handling) | tier: High
    #[test]
    fn commutativity_mul(a in representative(), b in representative()) {
        // WHY: a * b == b * a.  Fails if schoolbook multiply has asymmetric operand handling.
        let ab = mul(parse_or_zero(&a), parse_or_zero(&b));
        let ba = mul(parse_or_zero(&b), parse_or_zero(&a));
        prop_assert_eq!(d128_fmt(ab), d128_fmt(ba),
            "commutativity violated: {} * {}", a, b);
    }

    // bug: decimal128 mul by one changes the other operand's value | tier: High
    #[test]
    fn identity_mul(a in representative()) {
        // WHY: a * 1 == a.
        let a_bits = parse_or_zero(&a);
        let one = parse("1").unwrap();
        let result = mul(a_bits, one);
        let same = match (parse(&d128_fmt(a_bits)), parse(&d128_fmt(result))) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
        prop_assert!(same, "a * 1 != a for a={}: got {}", a, d128_fmt(result));
    }

    // bug: decimal128 format output does not re-parse to a numerically equal value, so a literal cannot survive parse, format, parse | tier: High
    #[test]
    fn round_trip(a in representative()) {
        // WHY: format(parse(s)) must produce a string that re-parses to a numerically
        // equal value.  parse("1e5") and parse("100000") produce different BID bit
        // patterns (different scale) — that is correct IEEE 754 behavior.  We check
        // NUMERIC equality, not bit equality, using compare().
        let bits = parse_or_zero(&a);
        let formatted = d128_fmt(bits);
        let reparsed = parse(&formatted);
        prop_assert!(reparsed.is_some(),
            "round-trip parse failed for formatted={}", formatted);
        prop_assert!(compare(bits, reparsed.unwrap()) == 0,
            "round-trip mismatch: {} -> {} -> numeric values differ", a, formatted);
    }
}
