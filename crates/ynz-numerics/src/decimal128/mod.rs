pub mod bits;
pub mod format;
pub mod ops;
pub mod parse;
pub(crate) mod wide;

/// Raw BID bit pattern representing a decimal128 value.
pub type Decimal128Bits = u128;
