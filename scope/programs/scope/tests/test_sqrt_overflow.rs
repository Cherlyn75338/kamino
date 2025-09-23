// Test to demonstrate sqrt_price_to_x64_price vulnerability with overflow-checks=true

#[cfg(test)]
mod tests {
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
    fn sqrt_price_to_x64_price_vulnerable(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> U192 {
        let sqrt_price = U256::from(sqrt_price);
        let price = (sqrt_price * sqrt_price) >> U256::from(64);
        let price_u256 = if decimals_a >= decimals_b {
            price * U256::from(ten_pow((decimals_a - decimals_b) as u32))
        } else {
            price / U256::from(ten_pow((decimals_b - decimals_a) as u32))
        };
        debug_assert_eq!(price_u256.0[3], 0, "price overflow: {:?}", price_u256);
        // THIS IS THE VULNERABILITY: Silent truncation by dropping the high limb
        U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]])
    }

    /// Fixed version that returns an error on overflow
    fn sqrt_price_to_x64_price_fixed(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> Result<U192, String> {
        let sqrt_price = U256::from(sqrt_price);
        let price = (sqrt_price * sqrt_price) >> U256::from(64);
        let price_u256 = if decimals_a >= decimals_b {
            price * U256::from(ten_pow((decimals_a - decimals_b) as u32))
        } else {
            price / U256::from(ten_pow((decimals_b - decimals_a) as u32))
        };
        
        // PROPER FIX: Check for overflow and return error
        if price_u256.0[3] != 0 {
            return Err("Math overflow: price exceeds U192 range".to_string());
        }
        
        Ok(U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]]))
    }

    /// Helper to check if overflow would occur
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

    #[test]
    fn test_overflow_with_max_sqrt_and_delta_1() {
        // This test proves overflow occurs with Δ=1 and max sqrt_price
        let sqrt_price = u128::MAX;
        let decimals_a = 7;
        let decimals_b = 6;
        
        let (price_u256, overflow) = compute_price_u256_and_overflow(sqrt_price, decimals_a, decimals_b);
        
        assert!(overflow, "Overflow should occur with max sqrt_price and Δ=1");
        assert_ne!(price_u256.0[3], 0, "High limb should be non-zero");
        
        // In vulnerable version, this creates a truncated result
        let truncated_result = sqrt_price_to_x64_price_vulnerable(sqrt_price, decimals_a, decimals_b);
        
        // The truncation happens silently - no panic even with overflow-checks=true
        // because it's not arithmetic overflow, it's deliberate data loss
        assert_eq!(truncated_result.0[0], price_u256.0[0]);
        assert_eq!(truncated_result.0[1], price_u256.0[1]);
        assert_eq!(truncated_result.0[2], price_u256.0[2]);
        // High limb is lost!
        
        // Fixed version would return an error
        let fixed_result = sqrt_price_to_x64_price_fixed(sqrt_price, decimals_a, decimals_b);
        assert!(fixed_result.is_err(), "Fixed version should return error on overflow");
    }

    #[test]
    fn test_overflow_with_large_decimal_difference() {
        // With Δ=12, overflow occurs at much lower sqrt_price
        let sqrt_price = 1u128 << 108; // 2^108
        let decimals_a = 18;
        let decimals_b = 6;
        
        let (_, overflow) = compute_price_u256_and_overflow(sqrt_price, decimals_a, decimals_b);
        assert!(overflow, "Overflow should occur with sqrt_price=2^108 and Δ=12");
    }

    #[test]
    fn test_inversion_attack_vector() {
        // Small sqrt_price becomes huge after inversion
        let small_sqrt_price = 2u128;
        
        // Simulate the inversion in sqrt_price_to_price when a_to_b=false
        let inverted_sqrt_price_u192 = (U192::one() << 128) / small_sqrt_price;
        let inverted_as_u128 = inverted_sqrt_price_u192.as_u128();
        
        // Expected value is 2^128 / 2 = 2^127
        let expected = (1u128 << 128) / small_sqrt_price;
        
        // Check if as_u128() truncates (it shouldn't in this case)
        assert_eq!(inverted_as_u128, expected, "No truncation for this value");
        
        // But with swapped decimals, this large value can cause overflow
        let decimals_a = 6;
        let decimals_b = 7;
        let (_, overflow) = compute_price_u256_and_overflow(inverted_as_u128, decimals_b, decimals_a);
        assert!(overflow, "Inverted small price causes overflow");
    }

    #[test]
    fn test_extreme_inversion_truncation() {
        // When sqrt_price = 1, inversion gives 2^128 which doesn't fit in u128
        let tiny_sqrt_price = 1u128;
        
        let inverted_sqrt_price_u192 = (U192::one() << 128) / tiny_sqrt_price;
        let inverted_as_u128 = inverted_sqrt_price_u192.as_u128();
        
        // 2^128 truncates to 0 when converted to u128!
        assert_eq!(inverted_as_u128, 0, "2^128 truncates to 0 in as_u128()");
        
        // This would cause the price to become 0, which is completely wrong
    }

    #[test]
    fn test_overflow_thresholds() {
        // Test various decimal differences and their overflow thresholds
        struct TestCase {
            delta: u8,
            threshold_sqrt: u128,
            should_overflow: bool,
        }
        
        let test_cases = vec![
            TestCase {
                delta: 1,
                threshold_sqrt: u128::MAX / 3,
                should_overflow: true,
            },
            TestCase {
                delta: 6,
                threshold_sqrt: u128::MAX / 1000,
                should_overflow: true,
            },
            TestCase {
                delta: 12,
                threshold_sqrt: 1u128 << 108,
                should_overflow: true,
            },
            TestCase {
                delta: 18,
                threshold_sqrt: 1u128 << 98,
                should_overflow: true,
            },
        ];
        
        for tc in test_cases {
            let decimals_a = tc.delta + 6;
            let decimals_b = 6;
            
            let (_, overflow) = compute_price_u256_and_overflow(tc.threshold_sqrt, decimals_a, decimals_b);
            assert_eq!(
                overflow, tc.should_overflow,
                "Delta={} should {} at threshold",
                tc.delta,
                if tc.should_overflow { "overflow" } else { "not overflow" }
            );
        }
    }

    #[test]
    fn test_realistic_mainnet_scenario() {
        // Test with realistic mainnet token pairs
        
        // Scenario 1: BONK (5 decimals) vs ETH (18 decimals)
        let bonk_decimals = 5u8;
        let eth_decimals = 18u8;
        let delta = eth_decimals - bonk_decimals; // 13
        
        // With Δ=13, overflow occurs around sqrt_price ≈ 2^107
        let manipulated_sqrt = 1u128 << 107;
        let (_, overflow) = compute_price_u256_and_overflow(manipulated_sqrt, eth_decimals, bonk_decimals);
        assert!(overflow, "BONK/ETH pair vulnerable to overflow");
        
        // Scenario 2: Regular USDC (6 decimals) vs SOL (9 decimals)
        let usdc_decimals = 6u8;
        let sol_decimals = 9u8;
        let delta = sol_decimals - usdc_decimals; // 3
        
        // With Δ=3, need much higher sqrt_price
        let extreme_sqrt = u128::MAX / 30;
        let (_, overflow) = compute_price_u256_and_overflow(extreme_sqrt, sol_decimals, usdc_decimals);
        assert!(overflow, "Even USDC/SOL can overflow with extreme sqrt_price");
    }

    #[test]
    fn test_overflow_checks_doesnt_prevent_truncation() {
        // This test demonstrates that overflow-checks=true does NOT prevent
        // the vulnerability because it's not arithmetic overflow
        
        let sqrt_price = u128::MAX;
        let decimals_a = 10;
        let decimals_b = 6;
        
        let (price_u256, overflow) = compute_price_u256_and_overflow(sqrt_price, decimals_a, decimals_b);
        assert!(overflow);
        
        // Even with overflow-checks=true, this line doesn't panic:
        let _truncated = U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]]);
        
        // The truncation is EXPLICIT, not an arithmetic operation
        // overflow-checks only prevents wrapping in +, -, *, / operations
        // It does NOT prevent:
        // - Explicit truncation via array indexing
        // - as_u128() conversions that truncate
        // - Deliberate data loss operations
    }
}