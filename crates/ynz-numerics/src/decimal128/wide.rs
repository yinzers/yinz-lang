/// 256-bit unsigned integer arithmetic for decimal128 multiplication and division.
///
/// `U256 { hi, lo }` represents the value `hi × 2^128 + lo`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U256 {
    pub hi: u128,
    pub lo: u128,
}

impl U256 {
    pub fn from_u128(v: u128) -> Self {
        U256 { hi: 0, lo: v }
    }

    /// Schoolbook 128×128 → 256-bit multiply.
    pub fn from_mul(a: u128, b: u128) -> Self {
        let a_hi = a >> 64;
        let a_lo = a & u64::MAX as u128;
        let b_hi = b >> 64;
        let b_lo = b & u64::MAX as u128;

        let p00 = a_lo * b_lo; // bits [127:0]   of the 256-bit result
        let p01 = a_lo * b_hi; // bits [191:64]
        let p10 = a_hi * b_lo; // bits [191:64]
        let p11 = a_hi * b_hi; // bits [255:128]

        let (mid, mid_carry) = p01.overflowing_add(p10);
        let mid_lo = mid & u64::MAX as u128;
        let mid_hi = mid >> 64;
        let (lo, lo_carry) = p00.overflowing_add(mid_lo << 64);

        let hi = p11
            .wrapping_add(mid_hi)
            .wrapping_add(if lo_carry { 1 } else { 0 })
            .wrapping_add(if mid_carry { 1u128 << 64 } else { 0 });

        U256 { hi, lo }
    }

    // ── Comparison ──────────────────────────────────────────────────────

    /// Multiply U256 by a u128, returning the lower 256 bits (wrapping on overflow).
    /// Sufficient for our use case since all intermediate values fit in 226 bits.
    pub fn mul_u128(self, rhs: u128) -> U256 {
        let lo_product = U256::from_mul(self.lo, rhs);
        let hi_lo = self.hi.wrapping_mul(rhs); // only lower 128 bits of hi×rhs needed
        U256 {
            hi: lo_product.hi.wrapping_add(hi_lo),
            lo: lo_product.lo,
        }
    }

    pub fn le(self, rhs: U256) -> bool {
        self.hi < rhs.hi || (self.hi == rhs.hi && self.lo <= rhs.lo)
    }

    // ── Arithmetic ──────────────────────────────────────────────────────

    /// Saturating subtraction.  Panics in debug if `rhs > self`.
    pub fn sub(self, rhs: U256) -> U256 {
        let (lo, borrow) = self.lo.overflowing_sub(rhs.lo);
        let hi = self.hi - rhs.hi - if borrow { 1 } else { 0 };
        U256 { hi, lo }
    }

    /// Left-shift by exactly `n` bits (n < 256).
    pub fn shl(self, n: u32) -> U256 {
        if n == 0 {
            return self;
        }
        if n >= 256 {
            return U256::from_u128(0);
        }
        if n >= 128 {
            // Only the low part contributes to the high part after the shift
            let shifted_lo = if n >= 256 { 0 } else { self.lo << (n - 128) };
            return U256 {
                hi: shifted_lo,
                lo: 0,
            };
        }
        // 0 < n < 128
        let hi = (self.hi << n) | (self.lo >> (128 - n));
        let lo = self.lo << n;
        U256 { hi, lo }
    }

    /// Divide self by a u128 divisor; returns (quotient U256, remainder u128).
    ///
    /// Time: O(256) — binary long division, fixed 256 iterations regardless of value.
    /// Space: O(1).
    ///
    /// Perf note: this is the hot path for every decimal128 division.  For
    /// high-throughput numeric workloads (physics, ML) replace with Knuth Algorithm D
    /// (multi-word long division, O(n/m) where n/m = word counts) at the v0.4
    /// performance pass.  For M2 correctness first; this is fast enough for interactive use.
    pub fn div_rem(self, divisor: u128) -> (U256, u128) {
        assert!(divisor != 0, "U256::div_rem: divisor is zero");
        if self.hi == 0 {
            return (U256::from_u128(self.lo / divisor), self.lo % divisor);
        }

        let d = U256::from_u128(divisor);
        let mut q = U256::from_u128(0);
        let mut r = U256::from_u128(0);

        // Process all 256 bits of self from MSB to LSB.
        for i in (0..256u32).rev() {
            r = r.shl(1);
            // Bring down bit i of self into r
            let bit = if i >= 128 {
                (self.hi >> (i - 128)) & 1
            } else {
                (self.lo >> i) & 1
            };
            r.lo |= bit;

            if d.le(r) {
                r = r.sub(d);
                // Set bit i in q
                if i >= 128 {
                    q.hi |= 1u128 << (i - 128);
                } else {
                    q.lo |= 1u128 << i;
                }
            }
        }

        (q, r.lo) // r < divisor ≤ u128::MAX, so r.hi == 0
    }

    /// Number of decimal digits (≥ 1).
    ///
    /// Time: O(30) worst case — linear walk from 39 to max 69 in the 256-bit case.
    /// Space: O(1).
    ///
    /// Perf note: only called on the hi≠0 path (values > 2^128), which only
    /// occurs when multiplying two ~34-digit coefficients.  Acceptable for M2.
    pub fn decimal_digits(self) -> u32 {
        if self.hi == 0 {
            return super::bits::decimal_digits(self.lo);
        }
        // self ≥ 2^128 > 10^38; walk up from 39.
        let mut n = 39u32;
        loop {
            if n >= 69 {
                return 69;
            }
            let (p_hi, p_lo) = pow10_256(n);
            if self.hi < p_hi || (self.hi == p_hi && self.lo < p_lo) {
                return n;
            }
            n += 1;
        }
    }
}

/// Returns the 256-bit representation of 10^n as (hi, lo) for n in [39, 68].
fn pow10_256(n: u32) -> (u128, u128) {
    const P38: u128 = 100_000_000_000_000_000_000_000_000_000_000_000_000; // 10^38
    let mut val = U256::from_u128(P38);
    for _ in 39..=n {
        // val × 10: val.lo × 10 can exceed u128::MAX (e.g. for 10^38 × 10 = 10^39 > 2^128).
        // Use from_mul to get the correct carry into hi.
        let lo_prod = U256::from_mul(val.lo, 10);
        let hi_new = val.hi.wrapping_mul(10).wrapping_add(lo_prod.hi);
        val = U256 {
            hi: hi_new,
            lo: lo_prod.lo,
        };
    }
    (val.hi, val.lo)
}

#[cfg(test)]
mod tests {
    use super::super::bits::TEN_34;
    use super::*;

    #[test]
    fn mul_small() {
        // WHY: schoolbook multiply must be exact for trivially verifiable inputs.
        let r = U256::from_mul(2, 3);
        assert_eq!(r.hi, 0);
        assert_eq!(r.lo, 6);
    }

    #[test]
    fn mul_large_no_overflow() {
        let r = U256::from_mul(TEN_34, 1);
        assert_eq!(r.hi, 0);
        assert_eq!(r.lo, TEN_34);
    }

    #[test]
    fn mul_cross_128_boundary() {
        // u128::MAX × 2 = 2^129 − 2
        let r = U256::from_mul(u128::MAX, 2);
        assert_eq!(r.hi, 1);
        assert_eq!(r.lo, u128::MAX - 1);
    }

    #[test]
    fn div_rem_simple() {
        // WHY: basic correctness check; if this fails, all decimal division is wrong.
        let v = U256::from_u128(100);
        let (q, r) = v.div_rem(7);
        assert_eq!(q.lo, 14);
        assert_eq!(r, 2);
    }

    #[test]
    fn div_rem_large() {
        // WHY: (10^34)^2 / 10^34 must equal 10^34 exactly.
        // This exercises the full 256-bit division path used in decimal128 mul.
        let prod = U256::from_mul(TEN_34, TEN_34);
        let (q, r) = prod.div_rem(TEN_34);
        assert_eq!(r, 0, "remainder should be 0");
        assert_eq!(q.hi, 0, "quotient must fit in u128");
        assert_eq!(q.lo, TEN_34);
    }

    #[test]
    fn div_rem_large_with_remainder() {
        let prod = U256::from_mul(TEN_34, TEN_34);
        let divisor = TEN_34 - 1;
        let (q, r) = prod.div_rem(divisor);
        // Check: q * divisor + r == prod
        let check = U256::from_mul(q.lo, divisor);
        let (sum_lo, carry) = check.lo.overflowing_add(r);
        let sum_hi = check.hi + if carry { 1 } else { 0 };
        assert_eq!((sum_hi, sum_lo), (prod.hi, prod.lo));
    }

    #[test]
    fn shl_basic() {
        let v = U256::from_u128(1);
        let shifted = v.shl(128);
        assert_eq!(shifted.hi, 1);
        assert_eq!(shifted.lo, 0);
    }
}
