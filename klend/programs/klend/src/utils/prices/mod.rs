mod checks;
mod pyth;
mod scope;
mod switchboard;
mod types;
mod utils;

use anchor_lang::prelude::*;
use types::{Price, TimestampedPrice};

use self::{
    checks::get_validated_price, pyth::get_pyth_price_and_twap, scope::get_scope_price_and_twap,
    switchboard::get_switchboard_price_and_twap, types::TimestampedPriceWithTwap,
};
use crate::{utils::Fraction, LendingError, PriceStatusFlags, TokenInfo};


const MAX_CONFIDENCE_PERCENTAGE: u64 = 2u64;


const CONFIDENCE_FACTOR: u64 = 100 / MAX_CONFIDENCE_PERCENTAGE;

#[derive(Debug, Clone)]
pub struct GetPriceResult {
    pub price: Fraction,
    pub timestamp: u64,
    pub status: PriceStatusFlags,
}

pub fn get_price(
    token_info: &TokenInfo,
    pyth_price_account_info: Option<&AccountInfo>,
    switchboard_price_feed_info: Option<&AccountInfo>,
    switchboard_price_twap_info: Option<&AccountInfo>,
    scope_prices_info: Option<&AccountInfo>,
    clock: &Clock,
) -> Result<Option<GetPriceResult>> {
    let price = get_most_recent_price_and_twap(
        token_info,
        pyth_price_account_info,
        switchboard_price_feed_info,
        switchboard_price_twap_info,
        scope_prices_info,
        clock,
    )?;

    Ok(get_validated_price(price, token_info, clock.unix_timestamp))
}



fn get_most_recent_price_and_twap(
    token_info: &TokenInfo,
    pyth_price_account_info: Option<&AccountInfo>,
    switchboard_price_feed_info: Option<&AccountInfo>,
    switchboard_price_twap_info: Option<&AccountInfo>,
    scope_prices_info: Option<&AccountInfo>,
    clock: &Clock,
) -> Result<TimestampedPriceWithTwap> {
    let pyth_price = if token_info.pyth_configuration.is_enabled() {
        pyth_price_account_info.and_then(|a| get_pyth_price_and_twap(a).ok())
    } else {
        None
    };

    let switchboard_price_twap_info_opt = if token_info.is_twap_enabled() {
        switchboard_price_twap_info
    } else {
        None
    };

    let switchboard_price = if token_info.switchboard_configuration.is_enabled() {
        switchboard_price_feed_info.and_then(|a| {
            get_switchboard_price_and_twap(a, switchboard_price_twap_info_opt, clock).ok()
        })
    } else {
        None
    };

    let scope_price = if token_info.scope_configuration.is_enabled() {
        scope_prices_info
            .and_then(|a| get_scope_price_and_twap(a, &token_info.scope_configuration).ok())
    } else {
        None
    };

    // Prefer recent but also consistent: select the most recent within a deviation band from median
    let mut candidates: Vec<_> = [pyth_price, switchboard_price, scope_price]
        .into_iter()
        .flatten()
        .collect();

    // If no candidates, error
    if candidates.is_empty() {
        return Err(error!(LendingError::PriceNotValid));
    }

    // Compute median timestamp and use it to compute a deviation band for values
    candidates.sort_by_key(|c| c.price.timestamp);
    let median_idx = candidates.len() / 2;
    let _median_candidate = candidates[median_idx].clone();

    // Compute pairwise median price by loading values that are available; fallback to timestamp-only if load fails
    let mut loaded_prices = Vec::with_capacity(candidates.len());
    for c in &candidates {
        if let Ok(px) = (c.price.price_load)() {
            loaded_prices.push(px);
        }
    }
    // If at least two prices loaded, compute mid value; else keep original selection strategy
    let most_recent_price = if loaded_prices.len() >= 2 {
        loaded_prices.sort();
        let median_price = loaded_prices[loaded_prices.len() / 2];
        // Accept candidates within a configurable band (use heuristic max_twap_divergence_bps as band)
        let acceptable_bps = token_info.max_twap_divergence_bps.max(50); // minimum 50 bps band
        let is_within_band = |px: Fraction| -> bool {
            let diff = Fraction::abs_diff(px, median_price) * 10_000u128;
            diff < median_price * u128::from(acceptable_bps)
        };

        candidates
            .into_iter()
            .filter(|c| (c.price.price_load)().map(is_within_band).unwrap_or(false))
            .max_by_key(|c| c.price.timestamp)
    } else {
        candidates
            .into_iter()
            .reduce(|current, candidate| {
                if candidate.price.timestamp > current.price.timestamp {
                    candidate
                } else {
                    current
                }
            })
    };

    most_recent_price.ok_or_else(|| {
        msg!("No price feed available");
        error!(LendingError::PriceNotValid)
    })
}

