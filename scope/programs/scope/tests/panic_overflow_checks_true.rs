#[cfg(test)]
mod panic_overflow_checks_true {
    use scope::states::Price;
    use scope::utils::math::price_of_lamports_to_price_of_tokens;

    // With overflow-checks=true (as configured in this repo's release profile),
    // the unchecked mantissa inflation in price_of_lamports_to_price_of_tokens
    // will panic when adjust_exp >= 2 and lamport_value is near 1e18.

    #[test]
    #[should_panic]
    fn overflow_panics_with_checks_enabled_sol_usdc_adjust_exp_3() {
        // token_a_decimals = 9 (SOL), token_b_decimals = 6 (USDC)
        // lamport_exp = 0 -> adjust_exp = 9 - (0 + 6) = 3
        // lamport_value = 1e18 -> multiply by 10^3 overflows u64
        let lamport_price = Price {
            value: 1_000_000_000_000_000_000u64,
            exp: 0,
        };
        let _ = price_of_lamports_to_price_of_tokens(lamport_price, 9, 6);
    }

    #[test]
    #[should_panic]
    fn overflow_panics_with_checks_enabled_sol_usdc_adjust_exp_2() {
        // token_a_decimals = 9 (SOL), token_b_decimals = 6 (USDC)
        // lamport_exp = 1 -> adjust_exp = 9 - (1 + 6) = 2
        // lamport_value ≈ 9.5e17 -> multiply by 10^2 overflows u64
        let lamport_price = Price {
            value: 950_000_000_000_000_000u64,
            exp: 1,
        };
        let _ = price_of_lamports_to_price_of_tokens(lamport_price, 9, 6);
    }
}

