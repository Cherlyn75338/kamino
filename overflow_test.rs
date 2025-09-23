// Test to verify overflow conditions in sqrt_price_to_x64_price
// This demonstrates the vulnerability with concrete values

fn main() {
    // Simulate the vulnerable function
    fn sqrt_price_to_x64_price_vulnerable(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> (u64, u64, u64, u64) {
        // Simulate U256 as [u64; 4]
        let sqrt_price_u256 = [
            (sqrt_price & 0xFFFFFFFFFFFFFFFF) as u64,
            ((sqrt_price >> 64) & 0xFFFFFFFFFFFFFFFF) as u64,
            0,
            0
        ];
        
        // Calculate price = (sqrt_price * sqrt_price) >> 64
        let price_low = sqrt_price_u256[0] * sqrt_price_u256[0];
        let price_mid = sqrt_price_u256[0] * sqrt_price_u256[1] + sqrt_price_u256[1] * sqrt_price_u256[0];
        let price_high = sqrt_price_u256[1] * sqrt_price_u256[1];
        
        // Right shift by 64
        let price = [
            price_low >> 64 | (price_mid << 64),
            price_mid >> 64 | (price_high << 64),
            price_high >> 64,
            0
        ];
        
        // Apply decimal scaling
        let ten_pow = 10_u128.pow((decimals_a - decimals_b) as u32);
        let ten_pow_u256 = [
            (ten_pow & 0xFFFFFFFFFFFFFFFF) as u64,
            ((ten_pow >> 64) & 0xFFFFFFFFFFFFFFFF) as u64,
            0,
            0
        ];
        
        // Multiply price * ten_pow
        let result = multiply_u256(price, ten_pow_u256);
        
        (result[0], result[1], result[2], result[3])
    }
    
    fn multiply_u256(a: [u64; 4], b: [u64; 4]) -> [u64; 4] {
        // Simplified multiplication - in reality this would be more complex
        // but this demonstrates the concept
        let mut result = [0u64; 4];
        
        for i in 0..4 {
            for j in 0..4 {
                if i + j < 4 {
                    let product = a[i] as u128 * b[j] as u128;
                    let carry = product >> 64;
                    let low = product as u64;
                    
                    if i + j < 4 {
                        let (sum, overflow) = result[i + j].overflowing_add(low);
                        result[i + j] = sum;
                        
                        if i + j + 1 < 4 && (overflow || carry > 0) {
                            let (sum2, _) = result[i + j + 1].overflowing_add(carry as u64 + if overflow { 1 } else { 0 });
                            result[i + j + 1] = sum2;
                        }
                    }
                }
            }
        }
        
        result
    }
    
    // Test cases
    println!("Testing overflow conditions:");
    
    // Case 1: Large sqrt_price with high decimal difference
    let sqrt_price = u128::MAX;
    let decimals_a = 30;
    let decimals_b = 0;
    
    let result = sqrt_price_to_x64_price_vulnerable(sqrt_price, decimals_a, decimals_b);
    println!("sqrt_price: {}, decimals_a: {}, decimals_b: {}", sqrt_price, decimals_a, decimals_b);
    println!("Result: [0]: {}, [1]: {}, [2]: {}, [3]: {}", result.0, result.1, result.2, result.3);
    println!("Overflow detected: {}", result.3 != 0);
    
    // Case 2: Moderate sqrt_price with high decimal difference
    let sqrt_price = 1u128 << 100; // 2^100
    let decimals_a = 18;
    let decimals_b = 0;
    
    let result = sqrt_price_to_x64_price_vulnerable(sqrt_price, decimals_a, decimals_b);
    println!("\nsqrt_price: {}, decimals_a: {}, decimals_b: {}", sqrt_price, decimals_a, decimals_b);
    println!("Result: [0]: {}, [1]: {}, [2]: {}, [3]: {}", result.0, result.1, result.2, result.3);
    println!("Overflow detected: {}", result.3 != 0);
    
    // Case 3: Calculate threshold for overflow
    // For overflow: sqrt_price^2 * 10^(decimals_a - decimals_b) >= 2^192
    // sqrt_price >= sqrt(2^192 / 10^(decimals_a - decimals_b))
    
    for decimal_diff in [18, 24, 30] {
        let threshold = ((1u128 << 192) / 10_u128.pow(decimal_diff) as u128) as f64;
        let sqrt_threshold = threshold.sqrt();
        println!("\nDecimal difference: {}, Threshold sqrt_price: {:.2e}", decimal_diff, sqrt_threshold);
        println!("u128::MAX: {:.2e}", u128::MAX as f64);
        println!("Overflow possible: {}", sqrt_threshold < u128::MAX as f64);
    }
}