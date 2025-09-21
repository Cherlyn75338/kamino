use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};

fn ten_pow(exponent: u32) -> u128 {
    match exponent {
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
        _ => panic!("no support for exponent: {exponent}"),
    }
}

fn check_confidence_interval(price_value: u128, price_exp: u32, deviation: u128, deviation_exp: u32, tolerance_factor: u32) -> bool {
    let common_exp = u32::min(price_exp, deviation_exp);
    let price_scaled = price_value.wrapping_mul(ten_pow(deviation_exp - common_exp));
    let deviation_scaled = deviation
        .wrapping_mul(tolerance_factor as u128)
        .wrapping_mul(ten_pow(price_exp - common_exp));
    price_scaled > deviation_scaled
}

fn big_check(price_value: u128, price_exp: u32, deviation: u128, deviation_exp: u32, tolerance_factor: u32) -> bool {
    let ten = BigUint::from(10u32);
    let price_scaled = BigUint::from(price_value) * ten.pow(deviation_exp);
    let deviation_scaled = BigUint::from(deviation) * BigUint::from(tolerance_factor) * ten.pow(price_exp);
    price_scaled > deviation_scaled
}

#[derive(Clone, Copy, Debug)]
struct Price { value: u64, exp: u64 }

fn price_of_lamports_to_price_of_tokens(lamport_price: Price, token_a_decimals: u64, token_b_decimals: u64) -> Price {
    let Price { value: lamport_value, exp: lamport_exp } = lamport_price;
    if lamport_exp + token_b_decimals >= token_a_decimals {
        let exp = lamport_exp + token_b_decimals - token_a_decimals;
        Price { value: lamport_value, exp }
    } else {
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        let value = lamport_value.wrapping_mul(10_u64.pow(adjust_exp as u32));
        Price { value, exp: 0 }
    }
}

fn most_recent_of_dos(now: u64, entries: &[(Price, u64)], sources_max_age_s: u64) -> Result<(Price, u64), &'static str> {
    let mut most_recent_ts = 0u64;
    let mut most_recent = Price { value: 0, exp: 0 };
    for (price, ts) in entries.iter().copied() {
        if now.saturating_sub(ts) > sources_max_age_s {
            return Err("MostRecentOfMaxAgeViolated");
        }
        if ts > most_recent_ts {
            most_recent_ts = ts;
            most_recent = price;
        }
    }
    Ok((most_recent, most_recent_ts))
}

fn main() {
    // 1) Confidence interval overflow PoC
    let price_value = u128::MAX / 2;
    let price_exp = 30u32;
    let deviation = (u128::MAX / 4) + 1;
    let deviation_exp = 0u32;
    let tolerance_factor = 2u32;

    let big = big_check(price_value, price_exp, deviation, deviation_exp, tolerance_factor);
    let small = check_confidence_interval(price_value, price_exp, deviation, deviation_exp, tolerance_factor);
    println!("confidence_overflow_divergence: big_ok={} small_ok={}", big, small);

    // 2) lamports->tokens overflow PoC
    let lamport_price = Price { value: u64::MAX, exp: 0 };
    let p = price_of_lamports_to_price_of_tokens(lamport_price, 30, 0);
    let expected_big = BigUint::from(u64::MAX as u128) * BigUint::from(10u32).pow(30);
    // Compare lower 64 bits against expected big value's lower limb
    let expected_low64 = if expected_big.is_zero() { 0 } else { expected_big.to_u64_digits().first().copied().unwrap_or(0) };
    println!("lamports_to_tokens_overflow: result_value={} expected_low64_limb={}", p.value, expected_low64);

    // 3) MostRecentOf DoS PoC
    let now = 1_000_000u64;
    let entries = vec![
        (Price { value: 100, exp: 2 }, now - 100),
        (Price { value: 101, exp: 2 }, now - 10_000), // stale
    ];
    let res = most_recent_of_dos(now, &entries, 300);
    println!("most_recent_of_dos: {:?}", res.err());
}

