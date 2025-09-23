#[cfg(test)]
mod overflow_checks_true_panic {
    use scope::states::Price;
    use scope::utils::math::price_of_lamports_to_price_of_tokens;

    // This test demonstrates that with overflow-checks=true in the release profile,
    // the unchecked multiplication in price_of_lamports_to_price_of_tokens will panic
    // on overflow rather than silently wrap.
    //
    // Scenario: SOL (9 decimals) vs USDC (6 decimals), lamport_exp = 0, value = 1e18
    // adjust_exp = 9 - (0 + 6) = 3 → multiply by 10^3, which overflows u64.
    #[test]
    #[should_panic]
    fn panic_on_overflow_with_checks_enabled() {
        let lamport_price = Price { value: 1_000_000_000_000_000_000, exp: 0 }; // 1e18
        let _ = price_of_lamports_to_price_of_tokens(lamport_price, 9, 6);
    }
}

