// PoC for Scope math overflow vulnerabilities
// This reproduces the vulnerable math functions from scope to demonstrate overflow conditions

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

// Vulnerable confidence interval check - reproduces the exact code from scope
pub fn check_confidence_interval_vulnerable(
    price_value: u128,
    price_exp: u32,
    deviation: u128,
    deviation_exp: u32,
    tolerance_factor: u32,
) -> Result<(), String> {
    // We return an error if price <= deviation * tolerance
    // price_value / 10^price_exp <= deviation_value * tolerance / 10^deviation_exp
    // price * 10^deviation_exp <= deviation * tolerance * 10^price_exp

    // avoid useless overflows simplify the exponents
    let common_exp = u32::min(price_exp, deviation_exp);

    // VULNERABLE: These multiplications can overflow u128
    let price_scaled = price_value * ten_pow(deviation_exp - common_exp);
    let deviation_scaled = deviation * u128::from(tolerance_factor) * ten_pow(price_exp - common_exp);

    if price_scaled <= deviation_scaled {
        return Err("ConfidenceIntervalCheckFailed".to_string());
    }

    Ok(())
}

// Safe version using checked arithmetic
pub fn check_confidence_interval_safe(
    price_value: u128,
    price_exp: u32,
    deviation: u128,
    deviation_exp: u32,
    tolerance_factor: u32,
) -> Result<(), String> {
    let common_exp = u32::min(price_exp, deviation_exp);

    // Use checked arithmetic to detect overflows
    let price_scaled = price_value
        .checked_mul(ten_pow(deviation_exp - common_exp))
        .ok_or("Price scaling overflow")?;
    
    let deviation_scaled = deviation
        .checked_mul(u128::from(tolerance_factor))
        .ok_or("Deviation tolerance overflow")?
        .checked_mul(ten_pow(price_exp - common_exp))
        .ok_or("Deviation scaling overflow")?;

    if price_scaled <= deviation_scaled {
        return Err("ConfidenceIntervalCheckFailed".to_string());
    }

    Ok(())
}

// Vulnerable asset_amount_to_usd from Jupiter LP (simplified)
pub fn asset_amount_to_usd_vulnerable(
    price_value: u64,
    price_exp: u8,
    token_amount: u64,
    token_decimals: u8,
) -> u128 {
    let price_value: u128 = price_value.into();
    let token_amount: u128 = token_amount.into();
    let price_decimals: u8 = price_exp;
    const POOL_VALUE_SCALE_DECIMALS: u8 = 6;

    // VULNERABLE: Unchecked multiplication can overflow u128
    if price_decimals + token_decimals > POOL_VALUE_SCALE_DECIMALS {
        let diff = price_decimals + token_decimals - POOL_VALUE_SCALE_DECIMALS;
        let nom = price_value * token_amount; // OVERFLOW RISK
        let denom = ten_pow(diff);
        nom / denom
    } else {
        let diff = POOL_VALUE_SCALE_DECIMALS - (price_decimals + token_decimals);
        price_value * token_amount * ten_pow(diff) // OVERFLOW RISK
    }
}

// Vulnerable price_of_lamports_to_price_of_tokens (simplified)
pub fn price_of_lamports_to_price_of_tokens_vulnerable(
    lamport_value: u64,
    lamport_exp: u64,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> (u64, u64) {
    if lamport_exp + token_b_decimals >= token_a_decimals {
        let exp = lamport_exp + token_b_decimals - token_a_decimals;
        (lamport_value, exp)
    } else {
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        // VULNERABLE: This can overflow u64
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        (value, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_interval_overflow() {
        println!("=== Testing Confidence Interval Overflow ===");
        
        // Test case that should cause overflow in vulnerable version
        let price_value = u128::MAX / 2; // Large price value
        let price_exp = 30;
        let deviation = u128::MAX / 4;
        let deviation_exp = 25;
        let tolerance_factor = 100;

        println!("Test parameters:");
        println!("  price_value: {}", price_value);
        println!("  price_exp: {}", price_exp);
        println!("  deviation: {}", deviation);
        println!("  deviation_exp: {}", deviation_exp);
        println!("  tolerance_factor: {}", tolerance_factor);
        
        // Test vulnerable version - this should overflow silently in release mode
        println!("\nTesting vulnerable version...");
        let vulnerable_result = std::panic::catch_unwind(|| {
            check_confidence_interval_vulnerable(price_value, price_exp, deviation, deviation_exp, tolerance_factor)
        });
        
        match vulnerable_result {
            Ok(result) => {
                println!("Vulnerable version result: {:?}", result);
                println!("⚠️  VULNERABILITY CONFIRMED: No overflow detection in vulnerable version");
            }
            Err(_) => {
                println!("Vulnerable version panicked (overflow in debug mode)");
            }
        }

        // Test safe version - this should detect overflow
        println!("\nTesting safe version...");
        let safe_result = check_confidence_interval_safe(price_value, price_exp, deviation, deviation_exp, tolerance_factor);
        println!("Safe version result: {:?}", safe_result);
        
        if safe_result.is_err() && safe_result.as_ref().unwrap_err().contains("overflow") {
            println!("✅ Safe version correctly detected overflow");
        }
    }

    #[test]
    fn test_jupiter_lp_overflow() {
        println!("\n=== Testing Jupiter LP AUM Overflow ===");
        
        // Test case that should cause overflow
        let price_value = u64::MAX;
        let price_exp = 0; // High precision price
        let token_amount = u64::MAX;
        let token_decimals = 18;
        
        println!("Test parameters:");
        println!("  price_value: {}", price_value);
        println!("  price_exp: {}", price_exp);
        println!("  token_amount: {}", token_amount);
        println!("  token_decimals: {}", token_decimals);

        // This should overflow u128 when multiplying price_value * token_amount
        let result = asset_amount_to_usd_vulnerable(price_value, price_exp, token_amount, token_decimals);
        println!("Result: {}", result);
        
        // Check if result is suspiciously small (indicating overflow wraparound)
        let expected_minimum = (u64::MAX as u128).pow(2) / ten_pow(18u32);
        if result < expected_minimum {
            println!("⚠️  VULNERABILITY CONFIRMED: Result {} is much smaller than expected minimum {}", 
                    result, expected_minimum);
            println!("    This indicates u128 overflow occurred and wrapped around");
        }
    }

    #[test]
    fn test_lamports_to_tokens_overflow() {
        println!("\n=== Testing Lamports to Tokens Overflow ===");
        
        // Test case that should cause overflow in u64 multiplication
        let lamport_value = u64::MAX / 1000; // Large but not max to avoid immediate overflow
        let lamport_exp = 0;
        let token_a_decimals = 18; // Large decimal difference
        let token_b_decimals = 0;
        
        println!("Test parameters:");
        println!("  lamport_value: {}", lamport_value);
        println!("  lamport_exp: {}", lamport_exp);
        println!("  token_a_decimals: {}", token_a_decimals);
        println!("  token_b_decimals: {}", token_b_decimals);
        
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        println!("  adjust_exp: {}", adjust_exp);
        println!("  10^adjust_exp: {}", 10_u64.pow(adjust_exp.try_into().unwrap()));
        
        // Test for overflow
        let overflow_check = lamport_value.checked_mul(10_u64.pow(adjust_exp.try_into().unwrap()));
        if overflow_check.is_none() {
            println!("⚠️  VULNERABILITY CONFIRMED: u64 overflow detected in checked multiplication");
        }
        
        // Test vulnerable version
        let result = std::panic::catch_unwind(|| {
            price_of_lamports_to_price_of_tokens_vulnerable(lamport_value, lamport_exp, token_a_decimals, token_b_decimals)
        });
        
        match result {
            Ok((value, exp)) => {
                println!("Vulnerable version result: value={}, exp={}", value, exp);
                // If value is much smaller than expected, it wrapped around
                if value < lamport_value {
                    println!("⚠️  VULNERABILITY CONFIRMED: Result wrapped around due to overflow");
                }
            }
            Err(_) => {
                println!("Vulnerable version panicked");
            }
        }
    }

    #[test]
    fn test_ten_pow_limits() {
        println!("\n=== Testing ten_pow Function Limits ===");
        
        // Test the maximum supported exponent
        let max_exp = 30u32;
        let max_value = ten_pow(max_exp);
        println!("ten_pow({}): {}", max_exp, max_value);
        println!("u128::MAX: {}", u128::MAX);
        println!("Remaining headroom: {}", u128::MAX - max_value);
        
        // Show how quickly we can reach overflow when multiplying with ten_pow results
        let test_values = [1000u128, 10000u128, 100000u128, 1000000u128];
        for value in test_values {
            for exp in [25u32, 26u32, 27u32, 28u32, 29u32, 30u32] {
                let ten_pow_val = ten_pow(exp);
                if let Some(result) = value.checked_mul(ten_pow_val) {
                    println!("{} * ten_pow({}) = {} (OK)", value, exp, result);
                } else {
                    println!("⚠️  {} * ten_pow({}) = OVERFLOW", value, exp);
                }
            }
        }
    }
}