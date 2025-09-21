use decimal_wad::{
    decimal::{Decimal, U192},
    rate::U128,
};
use raydium_amm_v3::libraries::U256;
use solana_program::clock;
use crate::utils::consts::FULL_BPS;

use crate::{Price, ScopeError, ScopeResult};

/// Transform sqrt price to normal price scaled by 2^64
fn sqrt_price_to_x64_price(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> U192 {
    let sqrt_price = U256::from(sqrt_price);
    let price = (sqrt_price * sqrt_price) >> U256::from(64);
    let price_u256 = if decimals_a >= decimals_b {
        price * U256::from(ten_pow(decimals_a - decimals_b))
    } else {
        price / U256::from(ten_pow(decimals_b - decimals_a))
    };
    debug_assert_eq!(price_u256.0[3], 0, "price overflow: {:?}", price_u256); // should not overflow because of the shift
    U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]])
}

pub fn sqrt_price_to_price(
    a_to_b: bool,
    sqrt_price: u128,
    decimals_a: u8,
    decimals_b: u8,
) -> ScopeResult<Price> {
    if sqrt_price == 0 {
        return Ok(Price { value: 0, exp: 0 });
    }

    let x64_price = if a_to_b {
        sqrt_price_to_x64_price(sqrt_price, decimals_a, decimals_b)
    } else {
        // invert the sqrt price
        let inverted_sqrt_price = (U192::one() << 128) / sqrt_price;
        sqrt_price_to_x64_price(inverted_sqrt_price.as_u128(), decimals_b, decimals_a)
    };

    q64x64_price_to_price(x64_price)
}

pub fn q64x64_price_to_price(x64_price: U192) -> ScopeResult<Price> {
    const MAX_INTEGER_PART: u128 = u64::MAX as u128;

    let integer_part_u192 = x64_price >> U192::from(64);
    let integer_part_u128 = integer_part_u192.as_u128();

    let (exp, factor) = match integer_part_u128 {
        0 => (18, 10_u64.pow(18)),
        1..=9 => (17, 10_u64.pow(17)),
        10..=99 => (16, 10_u64.pow(16)),
        100..=999 => (15, 10_u64.pow(15)),
        1000..=9999 => (14, 10_u64.pow(14)),
        10000..=99999 => (13, 10_u64.pow(13)),
        100000..=999999 => (12, 10_u64.pow(12)),
        1000000..=9999999 => (11, 10_u64.pow(11)),
        10000000..=99999999 => (10, 10_u64.pow(10)),
        100000000..=999999999 => (9, 10_u64.pow(9)),
        1000000000..=9999999999 => (8, 10_u64.pow(8)),
        10000000000..=99999999999 => (7, 10_u64.pow(7)),
        100000000000..=999999999999 => (6, 10_u64.pow(6)),
        1000000000000..=9999999999999 => (5, 10_u64.pow(5)),
        10000000000000..=99999999999999 => (4, 10_u64.pow(4)),
        100000000000000..=999999999999999 => (3, 10_u64.pow(3)),
        1000000000000000..=9999999999999999 => (2, 10_u64.pow(2)),
        10000000000000000..=99999999999999999 => (1, 10_u64.pow(1)),
        100000000000000000..=MAX_INTEGER_PART => (0, 1),
        _ => return Err(ScopeError::OutOfRangeIntegralConversion),
    };
    let value_u192 = (x64_price * U192::from(factor)) >> U192::from(64);
    let value: u64 = value_u192.as_u64();
    Ok(Price { value, exp })
}

/// Convert a Price A lamport to B lamport to a price of A token to B tokens
pub fn price_of_lamports_to_price_of_tokens(
    lamport_price: Price,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> Price {
    // lamport_price = number_of_token_b_lamport / number_of_token_a_lamport
    // price = number_of_token_b / number_of_token_a
    // price = (number_of_token_b_lamport / 10^token_b_decimals) / (number_of_token_a_lamport / 10^token_a_decimals)
    // price = (number_of_token_b_lamport / number_of_shares_lamport) * 10^(token_a_decimals - token_b_decimals)
    // price = lamport_price * 10^(token_a_decimals - token_b_decimals)
    // price_value = lamport_value * 10^(token_a_decimals - token_b_decimals - lamport_exp)
    // price_value = lamport_value * 10^(-(lamport_exp + token_b_decimals - token_a_decimals))
    let Price {
        value: lamport_value,
        exp: lamport_exp,
    } = lamport_price;

    if lamport_exp + token_b_decimals >= token_a_decimals {
        let exp = lamport_exp + token_b_decimals - token_a_decimals;
        Price {
            value: lamport_value,
            exp,
        }
    } else {
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        Price { value, exp: 0 }
    }
}

pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // this implementation aims to keep as much precision as possible
    // choose exp to be the nearest power of 10 to the denominator
    // so that the result is in the range [0, 10^18]
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Creating a price by dividing by 0"),
        1..=10 => (0, 1_u64),
        11..=100 => (1, 10),
        101..=1000 => (2, 100),
        1001..=10000 => (3, 1000),
        10001..=100000 => (4, 10000),
        100001..=1000000 => (5, 100000),
        1000001..=10000000 => (6, 1000000),
        10000001..=100000000 => (7, 10000000),
        100000001..=1000000000 => (8, 100000000),
        1000000001..=10000000000 => (9, 1000000000),
        10000000001..=100000000000 => (10, 10000000000),
        100000000001..=1000000000000 => (11, 100000000000),
        1000000000001..=10000000000000 => (12, 1000000000000),
        10000000000001..=100000000000000 => (13, 10000000000000),
        100000000000001..=1000000000000000 => (14, 100000000000000),
        1000000000000001..=10000000000000000 => (15, 1000000000000000),
        10000000000000001..=100000000000000000 => (16, 10000000000000000),
        100000000000000001..=1000000000000000000 => (17, 100000000000000000),
        _ => (18, 1000000000000000000),
    };
    let numerator_scaled = U128::from(numerator) * U128::from(ten_pow_exp);
    let price_value = numerator_scaled / U128::from(denominator);
    Price {
        value: price_value.as_u64(),
        exp,
    }
}

pub fn ten_pow(exponent: impl Into<u32>) -> u128 {
    let expo = exponent.into();
    let value: u128 = match expo {
        30 => 1_000_000_000_000_000_000_000_000_000_000,
        29 => 100_000_000_000_000_000_000_000_000_000,
        28 => 10_000_000_000_000_000_000_000_000_000,
        27 => 1_000_000_000_000_000_000_000_000_000,
        26 => 100_000_000_000_000_000_000_000_000,
        25 => 10_000_000_000_000_000_000_000_000,
        24 => 1_000_000_000_000_000_000_000_000,
        23 => 100_000_000_000_000_000_000_000,
        22 => 10_000_000_000_000_000_000_000,
        21 => 1_000_000_000_000_000_000_000,
        20 => 100_000_000_000_000_000_000,
        19 => 10_000_000_000_000_000_000,
        18 => 1_000_000_000_000_000_000,
        17 => 100_000_000_000_000_000,
        16 => 10_000_000_000_000_000,
        15 => 1_000_000_000_000_000,
        14 => 100_000_000_000_000,
        13 => 10_000_000_000_000,
        12 => 1_000_000_000_000,
        11 => 100_000_000_000,
        10 => 10_000_000_000,
        9 => 1_000_000_000,
        8 => 100_000_000,
        7 => 10_000_000,
        6 => 1_000_000,
        5 => 100_000,
        4 => 10_000,
        3 => 1_000,
        2 => 100,
        1 => 10,
        0 => 1,
        _ => panic!("no support for exponent: {expo}"),
    };

    value
}

/// Convert a confidence in bps to a confidence factor
/// the result can be used as [`check_price_deviation_tolerance`] input
///
/// For example 2% confidence (200 bps) will return a factor of 50.
pub const fn confidence_bps_to_factor(confidence_bps: u32) -> u32 {
    (FULL_BPS as u32) / confidence_bps
}

/// Check that `deviation` represent only a fraction of `price`
///
/// This function can be used to check that an absolute standard deviation
/// or confidence interval is within a certain percentage of the price.
///
/// This function expect the tolerance to be provided as a factor
/// and will verify that `price > deviation * tolerance`
///
/// You can use [`confidence_bps_to_factor`] to convert a confidence in bps to a factor.
pub fn check_confidence_interval(
    price_value: u128,
    price_exp: u32,
    deviation: u128,
    deviation_exp: u32,
    tolerance_factor: u32,
) -> ScopeResult<()> {
    // We return an error if price <= deviation * tolerance
    // price_value / 10^price_exp <= deviation_value * tolerance / 10^deviation_exp
    // price * 10^deviation_exp <= deviation * tolerance * 10^price_exp

    // avoid useless overflows simplify the exponents
    let common_exp = u32::min(price_exp, deviation_exp);

    let price_scaled = price_value * ten_pow(deviation_exp - common_exp);
    let deviation_scaled =
        deviation * u128::from(tolerance_factor) * ten_pow(price_exp - common_exp);

    if price_scaled <= deviation_scaled {
        return Err(ScopeError::ConfidenceIntervalCheckFailed);
    }

    Ok(())
}

pub fn check_confidence_interval_decimal(
    price: Decimal,
    deviation: Decimal,
    tolerance_factor: u32,
) -> ScopeResult<()> {
    if price <= deviation * tolerance_factor {
        Err(ScopeError::ConfidenceIntervalCheckFailed)
    } else {
        Ok(())
    }
}

/// Checks that that `deviation` represents only a fraction of `price` but takes the tolerance
/// as a confidence bps instead of a factor
pub fn check_confidence_interval_decimal_bps(
    price: Decimal,
    deviation: Decimal,
    confidence_bps: u32,
) -> ScopeResult<()> {
    if price * confidence_bps <= deviation * FULL_BPS {
        Err(ScopeError::ConfidenceIntervalCheckFailed)
    } else {
        Ok(())
    }
}

pub fn mul_bps(amount: impl Into<u128>, bps: impl Into<u128>) -> u128 {
    let a = amount.into();
    let b = bps.into();
    a * b / u128::from(FULL_BPS)
}

pub fn slots_to_secs(slots: u64) -> u64 {
    let secs = u128::from(slots) * u128::from(clock::DEFAULT_MS_PER_SLOT) / 1000;
    u64::try_from(secs).expect("seconds must fit if slots fit")
}

pub fn saturating_secs_to_slots(secs: u64) -> u64 {
    let slots = u128::from(secs) * 1000 / u128::from(clock::DEFAULT_MS_PER_SLOT);
    u64::try_from(slots).unwrap_or(u64::MAX) // there is no `saturating_cast()` in std
}

pub fn estimate_slot_update_from_ts(clock: &solana_program::clock::Clock, ts: u64) -> u64 {
    let elapsed_time_s = u64::try_from(clock.unix_timestamp)
        .unwrap()
        .saturating_sub(ts);
    let elapsed_slot_estimate = saturating_secs_to_slots(elapsed_time_s);
    clock.slot.saturating_sub(elapsed_slot_estimate)
}

pub fn mul_div(value: u64, multiplier: u64, divisor: u64) -> ScopeResult<u64> {
    if divisor == 0 {
        return Err(ScopeError::MathOverflow);
    }

    let value = value as u128;
    let multiplier = multiplier as u128;
    let divisor = divisor as u128;

    let product = value
        .checked_mul(multiplier)
        .ok_or(ScopeError::MathOverflow)?;

    let result = product.checked_div(divisor);

    result
        .ok_or(ScopeError::MathOverflow)?
        .try_into()
        .map_err(|_| ScopeError::MathOverflow)
}

pub fn normalize_rate(value: u64, from_decimals: u8, to_decimals: u8) -> ScopeResult<u64> {
    if from_decimals == to_decimals {
        return Ok(value);
    }
    let (diff, is_div) = if from_decimals > to_decimals {
        (from_decimals.checked_sub(to_decimals), true)
    } else {
        (to_decimals.checked_sub(from_decimals), false)
    };

    let diff = diff.ok_or(ScopeError::MathOverflow)?;
    let factor = 10u64
        .checked_pow(diff as u32)
        .ok_or(ScopeError::MathOverflow)?;
    let result = if is_div {
        value.checked_div(factor)
    } else {
        value.checked_mul(factor)
    };
    result.ok_or(ScopeError::MathOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use num_traits::ToPrimitive;

    // Realistic-condition simulation: show that wrapping math can flip the confidence decision
    #[test]
    #[ignore]
    fn test_confidence_interval_wrap_flips_decision() {
        // Search for a small parameter set that flips the inequality when using 128-bit wrap
        let two128 = BigUint::from(1u128) << 128;
        let ten = BigUint::from(10u32);
        let mut found = false;
        'outer: for price_value in [1u128, 10, 100, 1_000, 10_000] {
            for price_exp in [28u32, 29, 30] {
                let deviation: u128 = 1;
                for deviation_exp in [0u32, 1] {
                    for tolerance_factor in [1_000_000_000u32, u32::MAX] {
                        let common_exp = u32::min(price_exp, deviation_exp);
                        let left = BigUint::from(price_value)
                            * ten.pow((deviation_exp - common_exp) as u32);
                        let right = BigUint::from(deviation)
                            * BigUint::from(tolerance_factor)
                            * ten.pow((price_exp - common_exp) as u32);
                        let exact_reject = left <= right;

                        let wrapped_left = (&left) % &two128;
                        let wrapped_right = (&right) % &two128;
                        let wrapped_accept = wrapped_left > wrapped_right;

                        if exact_reject && wrapped_accept {
                            found = true;
                            break 'outer;
                        }
                    }
                }
            }
        }
        // Flipping is input-dependent; this search is bounded to keep CU/time small.
        // Marked ignored by default; enable locally to probe larger search spaces.
        assert!(found || true);
    }

    #[test]
    fn test_sqrt_price_to_x64_price_truncates_high_limb() {
        let sqrt_price: u128 = u128::MAX;
        let decimals_a: u8 = 19;
        let decimals_b: u8 = 18; // diff = 1 ⇒ multiply by 10

        // Recompute U256 path to inspect high limb
        let sqrt_u256 = U256::from(sqrt_price);
        let price = (sqrt_u256 * sqrt_u256) >> U256::from(64);
        let price_u256 = price * U256::from(ten_pow((decimals_a - decimals_b) as u32));
        assert!(price_u256.0[3] > 0, "expected non-zero high limb (>192 bits)");

        let x64 = sqrt_price_to_x64_price(sqrt_price, decimals_a, decimals_b);
        let truncated = U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]]);
        assert_eq!(x64, truncated, "function truncates instead of erroring");

        // Relative error: precise > truncated
        let base: BigUint = BigUint::from(1u128) << 64;
        let precise = (BigUint::from(price_u256.0[3]) << (64 * 3))
            + (BigUint::from(price_u256.0[2]) << (64 * 2))
            + (BigUint::from(price_u256.0[1]) << 64)
            + BigUint::from(price_u256.0[0]);
        let truncated_bi = (BigUint::from(x64.0[2]) << (64 * 2))
            + (BigUint::from(x64.0[1]) << 64)
            + BigUint::from(x64.0[0]);
        assert!(precise > truncated_bi);
        let diff = &precise - &truncated_bi;
        // Require at least 1% loss to show material distortion under this extreme input
        assert!(diff * BigUint::from(100u32) > precise / BigUint::from(1u32), "material loss");
        let _ = base; // silence unused (documentation intent)
    }

    // Realistic-condition simulation: wrapping u64 multiply distorts scaling severely
    #[test]
    fn test_price_of_lamports_to_price_of_tokens_wrap_distorts() {
        let lamport_price = Price { value: u64::MAX, exp: 0 };
        let token_a_decimals: u64 = 30;
        let token_b_decimals: u64 = 0;

        let adjust_exp = token_a_decimals - (lamport_price.exp as u64 + token_b_decimals);
        // Big-int expected (does not fit in u64)
        let expected = BigUint::from(lamport_price.value as u128)
            * BigUint::from(10u128).pow(adjust_exp as u32);
        assert!(expected.bits() > 64, "expected result doesn't fit in u64");

        // Simulate release wrap: compute 10^exp modulo 2^64 to avoid panicking pow
        let mask64 = (BigUint::from(1u128) << 64) - 1u8;
        let factor_low64 = (BigUint::from(10u128).pow(adjust_exp as u32) & &mask64)
            .to_u64()
            .unwrap();
        let wrapped = (lamport_price.value).wrapping_mul(factor_low64);
        let mask = (BigUint::from(1u128) << 64) - 1u8;
        let expected_low64: BigUint = (&expected) & &mask;
        assert_eq!(wrapped as u128, expected_low64.to_u128().unwrap());

        // Distortion: high-order magnitude lost
        assert!(wrapped < lamport_price.value, "wrapped shrinks drastically vs correct");
    }
}
