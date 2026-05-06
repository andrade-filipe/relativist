//! Pure-Rust Horner oracle (SPEC-27 v3 R16a').
//!
//! Computes `p(x) = sum(coeffs[i] * x^i for i in 0..coeffs.len())` via a
//! straight-line `BigUint` accumulator loop (no IC reduction). Used as the
//! reference semantics against which the `HornerCodec` is verified
//! (T7, T9, T9b property tests; T11 negative cross-check).
//!
//! Bounds enforced before evaluation:
//! - `coeffs.len() >= 1` (empty input → `OracleError::EmptyCoeffs`).
//! - `coeffs[i] <= MAX_CHURCH_NAT` for every `i`.
//! - `x <= MAX_CHURCH_NAT`.
//!
//! The cap value is sourced from `crate::encoding::church::MAX_CHURCH_NAT`
//! (single source of truth — Round 2 SC-007 / SC-013 closure). Code MUST NOT
//! hard-code `10_000`; if SPEC-14 R4 raises the cap, the oracle inherits it
//! automatically.

use num_bigint::BigUint;

use super::church::MAX_CHURCH_NAT;

/// Errors raised by `horner_serial` when the oracle's input bounds are
/// violated. Mirrors the encoder's `EncodeError::InvalidInput` family with an
/// explicit, structured representation so the negative cross-check
/// (T11 negative) can verify family correspondence.
///
/// SPEC-27 v3 R16a'.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OracleError {
    /// `coeffs.len() == 0` — Horner is undefined on the empty polynomial.
    #[error("empty coeffs")]
    EmptyCoeffs,

    /// `coeffs[idx] > max` — the indexed coefficient exceeds the public cap.
    #[error("coefficient at index {idx} = {value} exceeds cap (max {max})")]
    CoefficientOverflow { idx: usize, value: u64, max: u64 },

    /// `x > max` — the evaluation point exceeds the public cap.
    #[error("x = {value} exceeds cap (max {max})")]
    XOverflow { value: u64, max: u64 },
}

/// Pure-Rust Horner oracle (SPEC-27 v3 R16a').
///
/// Evaluates `p(x) = sum(coeffs[i] * x^i for i in 0..coeffs.len())` over
/// arbitrary-precision unsigned integers, returning the exact `BigUint`
/// result on success. Enforces the same input bounds as the encoder
/// (SPEC-27 v3 R12'): `coeffs.len() >= 1`, every `coeffs[i] <= MAX_CHURCH_NAT`,
/// and `x <= MAX_CHURCH_NAT`.
///
/// Validation order (matches the encoder, so error families correspond on the
/// same input — see T11 negative cross-check):
/// 1. empty `coeffs` → `EmptyCoeffs`,
/// 2. per-coefficient bound (first offender wins) → `CoefficientOverflow`,
/// 3. x bound → `XOverflow`.
///
/// Constant polynomial fast path: `coeffs.len() == 1` returns
/// `Ok(BigUint::from(coeffs[0]))` without entering the Horner loop.
pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError> {
    if coeffs.is_empty() {
        return Err(OracleError::EmptyCoeffs);
    }

    for (idx, &v) in coeffs.iter().enumerate() {
        if v > MAX_CHURCH_NAT {
            return Err(OracleError::CoefficientOverflow {
                idx,
                value: v,
                max: MAX_CHURCH_NAT,
            });
        }
    }

    if x > MAX_CHURCH_NAT {
        return Err(OracleError::XOverflow {
            value: x,
            max: MAX_CHURCH_NAT,
        });
    }

    // Constant polynomial: skip the loop entirely.
    if coeffs.len() == 1 {
        return Ok(BigUint::from(coeffs[0]));
    }

    // Horner recurrence: start at the leading coefficient, fold from
    // (n-1) down to 0: acc = acc * x + coeffs[k].
    let n = coeffs.len() - 1;
    let x_big = BigUint::from(x);
    let mut acc = BigUint::from(coeffs[n]);
    for k in (0..n).rev() {
        acc = acc * &x_big + BigUint::from(coeffs[k]);
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-0713-01: constant polynomial returns coeffs[0] regardless of x.
    #[test]
    fn horner_serial_constant_polynomial() {
        assert_eq!(horner_serial(&[42], 0).unwrap(), BigUint::from(42u64));
        assert_eq!(horner_serial(&[42], 7).unwrap(), BigUint::from(42u64));
        assert_eq!(horner_serial(&[0], 99).unwrap(), BigUint::from(0u64));
        assert_eq!(
            horner_serial(&[10_000], 0).unwrap(),
            BigUint::from(10_000u64)
        );
    }

    // UT-0713-02: canonical Horner case from the explainer doc, R11' ordering.
    #[test]
    fn horner_serial_canonical_explainer_case() {
        // p(x) = 3 + 2x + 5x^2 + x^3 at x=2: 3 + 4 + 20 + 8 = 35.
        assert_eq!(
            horner_serial(&[3, 2, 5, 1], 2).unwrap(),
            BigUint::from(35u64)
        );

        // Reverse coefficients confirm R11' ordering — coeffs[0] is constant
        // term, coeffs[len-1] is leading. Reversed gives 1 + 5*2 + 2*4 + 3*8 = 43.
        assert_eq!(
            horner_serial(&[1, 5, 2, 3], 2).unwrap(),
            BigUint::from(43u64)
        );

        // Same coefficients at x=0 → constant term.
        assert_eq!(
            horner_serial(&[3, 2, 5, 1], 0).unwrap(),
            BigUint::from(3u64)
        );
    }

    // UT-0713-03: sparse coefficients reduce correctly.
    #[test]
    fn horner_serial_sparse_coefficients() {
        // 1 + x^5 at x=10 = 1 + 100000 = 100001
        assert_eq!(
            horner_serial(&[1, 0, 0, 0, 0, 1], 10).unwrap(),
            BigUint::from(100_001u64)
        );

        // All zeros except first.
        assert_eq!(
            horner_serial(&[7, 0, 0, 0], 10).unwrap(),
            BigUint::from(7u64)
        );

        // All zeros.
        assert_eq!(
            horner_serial(&[0, 0, 0, 0], 7).unwrap(),
            BigUint::from(0u64)
        );
    }

    // UT-0713-04: T9 BigUint range — 25 coeffs of 1, x=10 -> (10^25 - 1)/9.
    #[test]
    fn horner_serial_biguint_range_25_coeffs() {
        let coeffs = vec![1u64; 25];
        let r = horner_serial(&coeffs, 10).unwrap();

        // 25 ones in a row.
        assert_eq!(r.to_string(), "1111111111111111111111111");
        assert!(
            r.bits() > 64,
            "T9 BigUint witness: expected bits > 64, got {}",
            r.bits()
        );
        assert!(
            r > BigUint::from(u64::MAX),
            "T9 result MUST strictly exceed u64::MAX"
        );
    }

    // UT-0713-05: T9b boundary — coeff=10000 AND x=10000.
    #[test]
    fn horner_serial_boundary_max_inputs() {
        let coeffs = [10_000u64, 10_000, 10_000, 10_000, 10_000];
        let r = horner_serial(&coeffs, 10_000).unwrap();

        // p(10000) = 10000 * (1 + 10000 + 10000^2 + 10000^3 + 10000^4)
        // Compute the expected value via independent BigUint arithmetic
        // (NOT a string literal — keeps the test self-checking).
        let x = BigUint::from(10_000u64);
        let mut x_pow = BigUint::from(1u64);
        let mut expected = BigUint::from(0u64);
        for _ in 0..5 {
            expected += &x_pow * BigUint::from(10_000u64);
            x_pow *= &x;
        }
        assert_eq!(r, expected);
        assert!(r.bits() > 64, "T9b BigUint witness: bits must exceed 64");
    }

    // UT-0713-06: empty coeffs -> EmptyCoeffs.
    #[test]
    fn horner_serial_empty_coeffs_returns_error() {
        assert_eq!(horner_serial(&[], 0), Err(OracleError::EmptyCoeffs));
        assert_eq!(horner_serial(&[], 99), Err(OracleError::EmptyCoeffs));

        // Empty wins over x overflow (validation order).
        assert_eq!(horner_serial(&[], 999_999), Err(OracleError::EmptyCoeffs));
    }

    // UT-0713-07: coefficient overflow.
    #[test]
    fn horner_serial_coefficient_overflow_returns_error() {
        assert_eq!(
            horner_serial(&[10_001], 0),
            Err(OracleError::CoefficientOverflow {
                idx: 0,
                value: 10_001,
                max: 10_000,
            })
        );

        // First offender (smallest index) wins.
        assert_eq!(
            horner_serial(&[1, 2, 99_999, 4], 5),
            Err(OracleError::CoefficientOverflow {
                idx: 2,
                value: 99_999,
                max: 10_000,
            })
        );

        // Boundary inclusive.
        assert!(horner_serial(&[10_000], 0).is_ok());
    }

    // UT-0713-08: x overflow.
    #[test]
    fn horner_serial_x_overflow_returns_error() {
        assert_eq!(
            horner_serial(&[1], 10_001),
            Err(OracleError::XOverflow {
                value: 10_001,
                max: 10_000,
            })
        );

        assert_eq!(
            horner_serial(&[1], u64::MAX),
            Err(OracleError::XOverflow {
                value: u64::MAX,
                max: 10_000,
            })
        );

        // Both coefficient overflow AND x overflow → coefficient wins
        // (validation order: empty → coeff → x).
        assert!(matches!(
            horner_serial(&[10_001], 10_001),
            Err(OracleError::CoefficientOverflow { .. })
        ));
    }
}
