use raydium_amm_v3::libraries::U256;
use decimal_wad::decimal::U192;

// Simulate the vulnerable function
fn sqrt_price_to_x64_price_vulnerable(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> U192 {
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

fn ten_pow(exponent: impl Into<u32>) -> u128 {
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
        19 => 1_000_000_000_000_000_000,
        18 => 100_000_000_000_000_000,
        17 => 10_000_000_000_000_000,
        16 => 1_000_000_000_000_000,
        15 => 100_000_000_000_000,
        14 => 10_000_000_000_000,
        13 => 1_000_000_000_000,
        12 => 100_000_000_000,
        11 => 10_000_000_000,
        10 => 1_000_000_000,
        9 => 100_000_000,
        8 => 10_000_000,
        7 => 1_000_000,
        6 => 100_000,
        5 => 10_000,
        4 => 1_000,
        3 => 100,
        2 => 10,
        1 => 1,
        0 => 1,
        _ => panic!("no support for exponent: {expo}"),
    };
    value
}

fn main() {
    println!("Testing sqrt_price_to_x64_price overflow behavior");
    
    // Test case 1: Large sqrt_price with high decimal difference
    let sqrt_price = u128::MAX;
    let decimals_a = 18u8;
    let decimals_b = 6u8;
    
    println!("\nTest 1: sqrt_price = u128::MAX, decimals_a = 18, decimals_b = 6");
    println!("Decimal difference: {}", decimals_a - decimals_b);
    
    let result = sqrt_price_to_x64_price_vulnerable(sqrt_price, decimals_a, decimals_b);
    println!("Result: {:?}", result);
    
    // Test case 2: Moderate sqrt_price with high decimal difference
    let sqrt_price = 1u128 << 100; // 2^100
    let decimals_a = 18u8;
    let decimals_b = 0u8;
    
    println!("\nTest 2: sqrt_price = 2^100, decimals_a = 18, decimals_b = 0");
    println!("Decimal difference: {}", decimals_a - decimals_b);
    
    let result = sqrt_price_to_x64_price_vulnerable(sqrt_price, decimals_a, decimals_b);
    println!("Result: {:?}", result);
    
    // Test case 3: Calculate threshold for overflow
    println!("\nTest 3: Overflow threshold analysis");
    for decimal_diff in [1, 6, 12, 18] {
        // For overflow: sqrt_price^2 * 10^(decimals_a - decimals_b) >= 2^192
        // sqrt_price >= sqrt(2^192 / 10^(decimals_a - decimals_b))
        let threshold = ((1u128 << 192) / ten_pow(decimal_diff) as u128) as f64;
        let sqrt_threshold = threshold.sqrt();
        println!("Decimal difference: {}, Threshold sqrt_price: {:.2e}", decimal_diff, sqrt_threshold);
        println!("u128::MAX: {:.2e}", u128::MAX as f64);
        println!("Overflow possible: {}", sqrt_threshold < u128::MAX as f64);
    }
    
    // Test case 4: Inversion attack - small sqrt_price becomes large after inversion
    println!("\nTest 4: Inversion attack analysis");
    let small_sqrt_price = 1u128; // Very small sqrt price
    let inverted = (U192::one() << 128) / small_sqrt_price;
    println!("Small sqrt_price: {}", small_sqrt_price);
    println!("Inverted sqrt_price: {}", inverted.as_u128());
    println!("This would cause overflow with any decimal difference >= 1");
}