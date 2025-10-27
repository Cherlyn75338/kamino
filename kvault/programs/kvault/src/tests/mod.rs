use proptest::prelude::*;

use kamino_lending::{fraction::Fraction, utils::FractionExtra};

use crate::{
    operations::vault_operations::{common, charge_fees},
    state::VaultState,
};

fn make_vault() -> VaultState { VaultState::default() }

proptest! {
    // Share price rounding inflation attempts: deposit then withdraw small loops shouldn't inflate balance
    #[test]
    fn share_price_rounding_cycles(
        initial in 1_000u64..100_000u64,
        cycles in 1u32..200u32,
        small in 1u64..100u64,
    ){
        let mut vault = make_vault();
        // initialize minimal fields
        vault.token_available = 0;
        vault.shares_issued = 0;
        let ts = 1_000_000u64;

        // initial deposit mints shares 1:1
        let shares_to_mint = common::get_shares_to_mint(Fraction::from(0u64), initial, 0).unwrap();
        common::deposit_into_vault(&mut vault, initial);
        common::mint_shares(&mut vault, shares_to_mint);
        common::update_prev_aum(&mut vault, Fraction::from(initial));

        let mut user_shares = shares_to_mint;

        for _ in 0..cycles {
            // deposit small amount
            let shares_to_mint = common::get_shares_to_mint(
                Fraction::from(vault.token_available), small, vault.shares_issued
            ).unwrap();
            common::deposit_into_vault(&mut vault, small);
            common::mint_shares(&mut vault, shares_to_mint);

            // withdraw proportional small amount
            let current_aum = Fraction::from(vault.token_available);
            let entitled = common::compute_user_total_received_on_withdraw(
                vault.shares_issued, current_aum, shares_to_mint
            );
            let to_burn = common::calculate_shares_to_burn(
                Fraction::from(entitled), vault.shares_issued, current_aum, shares_to_mint
            );
            // apply accounting
            common::withdraw_from_accounting(&mut vault, entitled, to_burn);
        }

        // Net effect should not exceed initial + small * cycles by more than a tiny epsilon due to rounding
        let max_expected = initial + small.saturating_mul(cycles as u64);
        prop_assert!(vault.token_available <= max_expected + 2, "vault grew unexpectedly from rounding");
    }
}

