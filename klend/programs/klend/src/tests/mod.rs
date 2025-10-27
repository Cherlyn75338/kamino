#![allow(clippy::arithmetic_side_effects)]

use std::panic;

use proptest::prelude::*;

use crate::{
    state::{reserve::ReserveCollateral, reserve::ReserveLiquidity, Reserve},
    utils::{borrow_rate_curve::BorrowRateCurve, Fraction, FractionExtra},
};

fn make_reserve() -> Reserve {
    Reserve::default()
}

fn make_liquidity() -> ReserveLiquidity {
    ReserveLiquidity::default()
}

fn make_collateral() -> ReserveCollateral {
    ReserveCollateral::new(crate::state::reserve::NewReserveCollateralParams {
        mint_pubkey: anchor_lang::prelude::Pubkey::default(),
        supply_vault: anchor_lang::prelude::Pubkey::default(),
        initial_collateral_supply: 0,
    })
}

proptest! {
    // Reserve invariants: total_supply should never exceed available + borrowed and should not panic
    #[test]
    fn total_supply_invariants(
        avail in 0u64..1_000_000_000u64,
        borrowed in 0u64..1_000_000_000u64,
        proto_fee in 0u64..1_000_000_000u64,
        ref_fee in 0u64..1_000_000_000u64,
        pending_ref in 0u64..1_000_000_000u64,
    ) {
        let mut liq = make_liquidity();
        liq.available_amount = avail;
        liq.borrowed_amount_sf = Fraction::from(borrowed).to_bits();
        liq.accumulated_protocol_fees_sf = Fraction::from(proto_fee).to_bits();
        liq.accumulated_referrer_fees_sf = Fraction::from(ref_fee).to_bits();
        liq.pending_referrer_fees_sf = Fraction::from(pending_ref).to_bits();

        let compute = || {
            let ts = liq.total_supply();
            // ts must not exceed available + borrowed
            let upper = Fraction::from(avail) + Fraction::from(borrowed);
            prop_assert!(ts <= upper, "total_supply larger than available+borrowed: ts={} upper={}", ts.to_display(), upper.to_display());
            Ok(())
        };

        // Ensure no panic
        let res = panic::catch_unwind(|| compute().unwrap());
        prop_assert!(res.is_ok(), "total_supply panicked (possible underflow/wrap)");
    }
}

#[test]
fn utilization_monotonic_with_borrow() {
    let mut reserve = make_reserve();
    // set simple liquidity state
    reserve.liquidity.available_amount = 1_000_000;
    reserve.liquidity.borrowed_amount_sf = Fraction::from(100_000u64).to_bits();
    let u0 = reserve.liquidity.utilization_rate();
    // Increase borrowed
    reserve.liquidity.borrowed_amount_sf = Fraction::from(200_000u64).to_bits();
    let u1 = reserve.liquidity.utilization_rate();
    assert!(u1 >= u0, "utilization not monotonic");
}

proptest! {
    // Exchange rate round-trip and ceil/floor consistency
    #[test]
    fn exchange_rate_roundtrip(
        coll_supply in 1u64..1_000_000u64,
        total_liq in 1u64..1_000_000u64,
        amt in 0u64..1_000_000u64,
    ) {
        let rate = crate::state::reserve::CollateralExchangeRate::from_supply_and_liquidity(
            coll_supply,
            Fraction::from(total_liq)
        );

        // liquidity->collateral->liquidity within 1
        let c = rate.liquidity_to_collateral(amt);
        let l_back = rate.collateral_to_liquidity(c);
        let diff1 = if l_back > amt { l_back - amt } else { amt - l_back };
        prop_assert!(diff1 <= 1, "l->c->l drift > 1: {}", diff1);

        // collateral->liquidity->collateral within 1
        let l = rate.collateral_to_liquidity(amt);
        let c_back = rate.liquidity_to_collateral(l);
        let diff2 = if c_back > amt { c_back - amt } else { amt - c_back };
        prop_assert!(diff2 <= 1, "c->l->c drift > 1: {}", diff2);

        // ceil variants should be >= floor variants
        let l_floor = rate.collateral_to_liquidity(amt);
        let l_ceil = rate.collateral_to_liquidity_ceil(amt);
        prop_assert!(l_ceil >= l_floor);

        let c_floor = rate.liquidity_to_collateral(amt);
        let c_ceil = rate.liquidity_to_collateral_ceil(amt);
        prop_assert!(c_ceil >= c_floor);
    }
}

#[test]
fn borrow_rate_boundary_monotonicity() {
    use crate::utils::borrow_rate_curve::CurvePoint;
    let points = [
        CurvePoint::new(0, 100),
        CurvePoint::new(5_000, 500),
        CurvePoint::new(10_000, 2_000),
    ];
    let curve = BorrowRateCurve::from_points(&points).unwrap();
    let u_below = Fraction::from_bps(4_999);
    let u_at = Fraction::from_bps(5_000);
    let u_above = Fraction::from_bps(5_001);
    let r_below = curve.get_borrow_rate(u_below).unwrap();
    let r_at = curve.get_borrow_rate(u_at).unwrap();
    let r_above = curve.get_borrow_rate(u_above).unwrap();
    assert!(r_at >= r_below, "non-monotone at boundary (below->at) {} < {}", r_at.to_display(), r_below.to_display());
    assert!(r_above >= r_at, "non-monotone at boundary (at->above) {} < {}", r_above.to_display(), r_at.to_display());
}

#[test]
fn oracle_gating_last_update_flags() {
    use crate::state::last_update::{LastUpdate, PriceStatusFlags, STALE_AFTER_SLOTS_ELAPSED};
    let slot0: u64 = 100;
    let mut lu = LastUpdate::new(slot0);
    // Missing flags: should be stale for ALL_CHECKS
    lu.update_slot(slot0, Some(PriceStatusFlags::PRICE_LOADED | PriceStatusFlags::PRICE_AGE_CHECKED));
    assert!(lu.is_stale(slot0, PriceStatusFlags::ALL_CHECKS).unwrap(), "should be stale without all checks");
    // With ALL flags: not stale in same slot
    lu.update_slot(slot0, Some(PriceStatusFlags::ALL_CHECKS));
    assert!(!lu.is_stale(slot0, PriceStatusFlags::ALL_CHECKS).unwrap(), "should not be stale with all checks same slot");
    // After STALE_AFTER_SLOTS_ELAPSED slots, becomes stale
    let later = slot0 + STALE_AFTER_SLOTS_ELAPSED;
    assert!(lu.is_stale(later, PriceStatusFlags::ALL_CHECKS).unwrap(), "should be stale after elapsed slots");
}

proptest! {
    // Compounded interest properties: result >= 1, non-decreasing with slots, non-negative variable component
    #[test]
    fn compounding_interest_properties(rate_pct in 0u64..500u64, slots in 0u64..10_000u64, prev_debt in 0u64..1_000_000u64, host_fixed_bps in 0u16..10_000u16) {
        use crate::state::reserve::approximate_compounded_interest;
        let rate = Fraction::from_percent(rate_pct as u128);
        let c = approximate_compounded_interest(rate, slots);
        prop_assert!(c >= Fraction::ONE);

        // check monotonicity in slots: c(slots+1) >= c(slots)
        let c_next = approximate_compounded_interest(rate, slots.saturating_add(1));
        prop_assert!(c_next >= c);

        // variable component non-negative: new_debt - prev_debt - fixed_fee >= 0
        let host_fixed = Fraction::from_bps(host_fixed_bps as u128);
        let c_total = approximate_compounded_interest(rate + host_fixed, slots);
        let c_fixed = approximate_compounded_interest(host_fixed, slots);
        let prev = Fraction::from(prev_debt);
        let new_debt = prev * c_total;
        let fixed_fee = prev * c_fixed - prev;
        let var_part = new_debt - prev - fixed_fee;
        prop_assert!(var_part >= Fraction::ZERO, "negative variable part");
    }
}

#[test]
fn exchange_rate_reset_with_liquidity_present() {
    // Simulate state where collateral supply is zero but liquidity > 0
    let mut reserve = make_reserve();
    reserve.collateral.mint_total_supply = 0;
    reserve.liquidity.available_amount = 1_000; // total_liquidity > 0
    reserve.liquidity.borrowed_amount_sf = Fraction::from(0u64).to_bits();
    reserve.liquidity.accumulated_protocol_fees_sf = 0;
    reserve.liquidity.accumulated_referrer_fees_sf = 0;
    reserve.liquidity.pending_referrer_fees_sf = 0;

    // Exchange rate should be initial 1:1 => deposit 1_000 mints 1_000 collateral
    let rate = reserve.collateral_exchange_rate();
    let minted = rate.liquidity_to_collateral(1_000);
    assert_eq!(minted, 1_000, "1:1 reset when supply=0 but liquidity>0");
}

