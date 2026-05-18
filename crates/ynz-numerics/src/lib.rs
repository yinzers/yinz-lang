/// IEEE 754 decimal128 implementation for the Yinz compiler.
///
/// No external runtime crates — correctness is enforced by conformance tests
/// against the IBM Hursley corpus and differential testing against Python `decimal`.
pub mod decimal128;

/// Arbitrary-precision decimal arithmetic for `number<N>` with N ∈ (34, 4096].
///
/// Uses digit-vector representation with half-even rounding at the given precision.
/// The decimal128 hardware path (N ≤ 34) is unaffected.
pub mod decimal_n;

pub use decimal128::bits::{
    decode, encode_finite, encode_infinity, encode_qnan, D128Kind, D128Value, EXPONENT_BIAS,
    MAX_COEFFICIENT, MAX_EXPONENT, MIN_EXPONENT, NEG_INF, NEG_ZERO, POS_INF, POS_ZERO, QUIET_NAN,
};
pub use decimal128::format::format;
pub use decimal128::ops::{abs, add, compare, div, mul, neg, sub};
pub use decimal128::parse::parse;
pub use decimal128::Decimal128Bits;
pub use decimal_n::BigNum;
