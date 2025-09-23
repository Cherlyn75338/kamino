// Test to verify sqrt_price_to_x64_price vulnerability behavior with overflow-checks=true

use decimal_wad::decimal::{Decimal, U192};
use raydium_amm_v3::libraries::U256;

/// Helper function to compute power of 10
fn ten_pow(exponent: u32) -> u128 {
    match exponent {
        0 => 1,
        1 => 10,
        2 => 100,
        3 => 1_000,
        4 => 10_000,
        5 => 100_000,
        6 => 1_000_000,
        7 => 10_000_000,
        8 => 100_000_000,
        9 => 1_000_000_000,
        10 => 10_000_000_000,
        11 => 100_000_000_000,
        12 => 1_000_000_000_000,
        13 => 10_000_000_000_000,
        14 => 100_000_000_000_000,
        15 => 1_000_000_000_000_000,
        16 => 10_000_000_000_000_000,
        17 => 100_000_000_000_000_000,
        18 => 1_000_000_000_000_000_000,
        19 => 10_000_000_000_000_000_000,
        20 => 100_000_000_000_000_000_000,
        21 => 1_000_000_000_000_000_000_000,
        22 => 10_000_000_000_000_000_000_000,
        23 => 100_000_000_000_000_000_000_000,
        24 => 1_000_000_000_000_000_000_000_000,
        25 => 10_000_000_000_000_000_000_000_000,
        26 => 100_000_000_000_000_000_000_000_000,
        27 => 1_000_000_000_000_000_000_000_000_000,
        28 => 10_000_000_000_000_000_000_000_000_000,
        29 => 100_000_000_000_000_000_000_000_000_000,
        30 => 1_000_000_000_000_000_000_000_000_000_000,
        _ => panic!("no support for exponent: {}", exponent),
    }
}

/// The vulnerable function from the codebase
fn sqrt_price_to_x64_price(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> U192 {
    let sqrt_price = U256::from(sqrt_price);
    let price = (sqrt_price * sqrt_price) >> U256::from(64);
    let price_u256 = if decimals_a >= decimals_b {
        price * U256::from(ten_pow((decimals_a - decimals_b) as u32))
    } else {
        price / U256::from(ten_pow((decimals_b - decimals_a) as u32))
    };
    debug_assert_eq!(price_u256.0[3], 0, "price overflow: {:?}", price_u256);
    U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]])
}

/// Test helper to check if overflow would occur
fn compute_price_u256_and_overflow(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> (U256, bool) {
    let sqrt_price = U256::from(sqrt_price);
    let price = (sqrt_price * sqrt_price) >> U256::from(64);
    let price_u256 = if decimals_a >= decimals_b {
        price * U256::from(ten_pow((decimals_a - decimals_b) as u32))
    } else {
        price / U256::from(ten_pow((decimals_b - decimals_a) as u32))
    };
    let overflow = price_u256.0[3] != 0;
    (price_u256, overflow)
}

fn main() {
    println!("Testing sqrt_price_to_x64_price vulnerability with overflow-checks=true\n");
    
    // Test 1: Maximum sqrt_price with decimal difference of 1
    println!("Test 1: Maximum sqrt_price with Δ=1");
    let sqrt_price = u128::MAX;
    let decimals_a = 7;
    let decimals_b = 6;
    let (price_u256, overflow) = compute_price_u256_and_overflow(sqrt_price, decimals_a, decimals_b);
    println!("  sqrt_price: {}", sqrt_price);
    println!("  decimals_a: {}, decimals_b: {}", decimals_a, decimals_b);
    println!("  price_u256.0[3] (high limb): {}", price_u256.0[3]);
    println!("  Overflow detected: {}", overflow);
    
    if overflow {
        println!("  Creating U192 from truncated limbs...");
        let truncated = U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]]);
        println!("  Truncated U192 created successfully!");
        println!("  This demonstrates SILENT TRUNCATION in release builds!");
    }
    
    // Test 2: Large sqrt_price with decimal difference of 12
    println!("\nTest 2: Large sqrt_price with Δ=12");
    let sqrt_price = 1u128 << 108; // 2^108
    let decimals_a = 18;
    let decimals_b = 6;
    let (price_u256, overflow) = compute_price_u256_and_overflow(sqrt_price, decimals_a, decimals_b);
    println!("  sqrt_price: {} (2^108)", sqrt_price);
    println!("  decimals_a: {}, decimals_b: {}", decimals_a, decimals_b);
    println!("  price_u256.0[3] (high limb): {}", price_u256.0[3]);
    println!("  Overflow detected: {}", overflow);
    
    // Test 3: Inversion attack - small sqrt_price becomes large after inversion
    println!("\nTest 3: Inversion attack (a_to_b=false path)");
    let small_sqrt_price = 2u128;
    let inverted_sqrt_price_u192 = (U192::one() << 128) / small_sqrt_price;
    println!("  Original sqrt_price: {}", small_sqrt_price);
    println!("  After inversion: U192 value");
    
    // Check if inverted value fits in u128
    let inverted_as_u128 = inverted_sqrt_price_u192.as_u128();
    let expected_inverted = (1u128 << 128) / small_sqrt_price;
    println!("  Expected inverted: {}", expected_inverted);
    println!("  After as_u128(): {}", inverted_as_u128);
    
    if inverted_as_u128 != expected_inverted {
        println!("  WARNING: as_u128() truncation detected!");
    }
    
    // Now check if this causes overflow
    let (price_u256, overflow) = compute_price_u256_and_overflow(inverted_as_u128, decimals_b, decimals_a);
    println!("  Testing with swapped decimals (b={}, a={})", decimals_b, decimals_a);
    println!("  Overflow after inversion: {}", overflow);
    
    // Test 4: Demonstrate that overflow-checks=true doesn't prevent truncation
    println!("\nTest 4: Confirming truncation behavior");
    println!("  overflow-checks=true prevents wrapping in arithmetic operations");
    println!("  BUT it does NOT prevent explicit truncation like:");
    println!("    U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]])");
    println!("  This is NOT an arithmetic overflow - it's deliberate data loss!");
    
    // Test 5: Calculate exact thresholds for various decimal differences
    println!("\nTest 5: Overflow thresholds for different decimal differences");
    for delta in [1, 6, 12, 18, 24, 30] {
        let threshold = calculate_overflow_threshold(delta);
        println!("  Δ={}: sqrt_price must be >= {} to cause overflow", delta, threshold);
        println!("       As fraction of u128::MAX: {:.2}%", 
                 (threshold as f64 / u128::MAX as f64) * 100.0);
    }
}

fn calculate_overflow_threshold(decimal_diff: u8) -> u128 {
    // Overflow occurs when: (sqrt_price^2 >> 64) * 10^Δ >= 2^192
    // Solving for sqrt_price: sqrt_price >= 2^128 / 10^(Δ/2)
    
    // For simplicity, we'll calculate approximate thresholds
    match decimal_diff {
        1 => u128::MAX / 3,  // ~2^127.4
        6 => u128::MAX / 1000,  // ~2^118
        12 => u128::MAX / 1_000_000,  // ~2^108
        18 => u128::MAX / 1_000_000_000,  // ~2^98
        24 => u128::MAX / 1_000_000_000_000,  // ~2^88
        30 => u128::MAX / 1_000_000_000_000_000,  // ~2^78
        _ => u128::MAX,
    }
}