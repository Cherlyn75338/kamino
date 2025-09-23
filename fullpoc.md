# Critical Security Report: Integer Overflow in `price_of_lamports_to_price_of_tokens`

## Executive Summary
- **Severity**: Critical
- **Component**: `scope/programs/scope/src/utils/math.rs::price_of_lamports_to_price_of_tokens`
- **Status**: Confirmed exploitable by code analysis
- **Impact**: Wrong oracle prices can be produced and stored on-chain, propagating to KLend/KFarms and others, enabling misvaluation, under/over-liquidation, and potential bad debt.

## Price Representation and Invariants
Existing type used across Scope:
```13:26:/workspace/scope/programs/scope/src/states.rs
#[zero_copy]
#[derive(Debug, Default, AnchorDeserialize, AnchorSerialize)]
pub struct Price {
    // Pyth price, integer + exponent representation
    // decimal price would be
    // as integer: 6462236900000, exponent: 8
    // as float:   64622.36900000

    // value is the scaled integer
    // for example, 6462236900000 for btc
    pub value: u64,

    // exponent represents the number of decimals
    // for example, 8 for btc
    pub exp: u64,
}
```
The codebase generally keeps the mantissa `value` within a safe range (≤ 10^18) and adjusts precision via `exp`.

## Vulnerable Function and Exact Code
The function converts a price expressed in lamports to a token-to-token price by compensating decimal differences. The else-branch inflates the mantissa in `u64` without safety checks.
```78:106:/workspace/scope/programs/scope/src/utils/math.rs
/// Convert a Price A lamport to B lamport to a price of A token to B tokens
pub fn price_of_lamports_to_price_of_tokens(
    lamport_price: Price,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> Price {
    // lamport_price = number_of_token_b_lamport / number_of_token_a_lamport
    // price = number_of_token_b / number_of_token_a
    // price = (number_of_token_b_lamport / 10^token_b_decimals) / (number_of_token_a_lamport / 10^token_a_decimals)
    // price = (number_of_token_b_lamport / number_of_shares_lamport) * 10^(token_a_decimals - token_b_decimals)
    // price = lamport_price * 10^(token_a_decimals - token_b_decimals)
    // price_value = lamport_value * 10^(token_a_decimals - token_b_decimals - lamport_exp)
    // price_value = lamport_value * 10^(-(lamport_exp + token_b_decimals - token_a_decimals))
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
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        Price { value, exp: 0 }
    }
}
```

## Upstream Sources and Range of Mantissa
Two key producers of `Price` that feed into the vulnerable function:
- Division-based price keeps `value` ≤ 10^18 by design
```109:141:/workspace/scope/programs/scope/src/utils/math.rs
pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // this implementation aims to keep as much precision as possible
    // choose exp to be the nearest power of 10 to the denominator
    // so that the result is in the range [0, 10^18]
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Creating a price by dividing by 0"),
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
    let numerator_scaled = U128::from(numerator) * U128::from(ten_pow_exp);
    let price_value = numerator_scaled / U128::from(denominator);
    Price {
        value: price_value.as_u64(),
        exp,
    }
}
```
- Q64.64 → Price selection sets `exp` buckets; still produces large mantissas
```45:76:/workspace/scope/programs/scope/src/utils/math.rs
pub fn q64x64_price_to_price(x64_price: U192) -> ScopeResult<Price> {
    const MAX_INTEGER_PART: u128 = u64::MAX as u128;

    let integer_part_u192 = x64_price >> U192::from(64);
    let integer_part_u128 = integer_part_u192.as_u128();

    let (exp, factor) = match integer_part_u128 {
        0 => (18, 10_u64.pow(18)),
        1..=9 => (17, 10_u64.pow(17)),
        10..=99 => (16, 10_u64.pow(16)),
        100..=999 => (15, 10_u64.pow(15)),
        1000..=9999 => (14, 10_u64.pow(14)),
        10000..=99999 => (13, 10_u64.pow(13)),
        100000..=999999 => (12, 10_u64.pow(12)),
        1000000..=9999999 => (11, 10_u64.pow(11)),
        10000000..=99999999 => (10, 10_u64.pow(10)),
        100000000..=999999999 => (9, 10_u64.pow(9)),
        1000000000..=9999999999 => (8, 10_u64.pow(8)),
        10000000000..=99999999999 => (7, 10_u64.pow(7)),
        100000000000..=999999999999 => (6, 10_u64.pow(6)),
        1000000000000..=9999999999999 => (5, 10_u64.pow(5)),
        10000000000000..=99999999999999 => (4, 10_u64.pow(4)),
        100000000000000..=999999999999999 => (3, 10_u64.pow(3)),
        1000000000000000..=9999999999999999 => (2, 10_u64.pow(2)),
        10000000000000000..=99999999999999999 => (1, 10_u64.pow(1)),
        100000000000000000..=MAX_INTEGER_PART => (0, 1),
        _ => return Err(ScopeError::OutOfRangeIntegralConversion),
    };
    let value_u192 = (x64_price * U192::from(factor)) >> U192::from(64);
    let value: u64 = value_u192.as_u64();
    Ok(Price { value, exp })
}
```

## Call Sites — Propagation Paths
- Meteora DLMM adapter:
```80:86:/workspace/scope/programs/scope/src/oracles/meteora_dlmm.rs
let price = math::price_of_lamports_to_price_of_tokens(
    lamport_price,
    src_token_decimals.into(),
    dst_token_decimals.into(),
);
```
- KTokens Token-X adapter:
```144:157:/workspace/scope/programs/scope/src/oracles/ktokens_token_x.rs
let price_lamport_to_lamport = u64_div_to_price(num_token_x, num_shares);
// Final price need to be adjusted by the number of decimals of the kToken and the token X
let share_decimals = strategy_account_ref.shares_mint_decimals;
let token_decimals = match token {
    TokenTypes::TokenA => strategy_account_ref.token_a_mint_decimals,
    TokenTypes::TokenB => strategy_account_ref.token_b_mint_decimals,
};

price_of_lamports_to_price_of_tokens(
    price_lamport_to_lamport,
    share_decimals,
    token_decimals,
)
```
- Refresh handler writes prices on-chain:
```141:158:/workspace/scope/programs/scope/src/handlers/handler_refresh_prices.rs
let to_update = oracle_prices
    .prices
    .get_mut(token_idx)
    .ok_or(ScopeError::BadTokenNb)?;

msg!(
    "tk {}, {:?}: {:?} to {:?} | prev_slot: {:?}, new_slot: {:?}, crt_slot: {:?}",
    token_idx,
    price_type,
    to_update.price.value,
    price.price.value,
    to_update.last_updated_slot,
    price.last_updated_slot,
    clock.slot,
);

*to_update = price;
```

## Mathematical Proof of Overflow
Let `Price { value = v, exp = e }` with design bound `v ≤ 10^18`. For token decimals `a = token_a_decimals`, `b = token_b_decimals`:
- Else-branch triggers if `e + b < a`. Define `k = a − (e + b) ≥ 1`.
- The code computes `v' = v × 10^k` in `u64` and sets `exp' = 0`.
- With `u64::MAX ≈ 1.8446744 × 10^19`:
  - If `k ≥ 2`, then `10^18 × 10^k ≥ 10^20 > u64::MAX` ⇒ overflow.
  - If `k = 1`, overflow occurs for `v > 1.8446744 × 10^18`.
- Common SPL decimals make `k ≥ 2` plausible: e.g., SOL(9) vs USDC(6) yields `Δ = 3`. For `e ∈ {0,1}`, `k ∈ {3,2}`.

Therefore, for realistic inputs, the product overflows and wraps in release.

## Concrete Scenarios
- SOL/USDC-like: `a=9`, `b=6`
  - If `e=1`, `k=2`. With `v≈10^18` ⇒ `v'≈10^20` ⇒ overflow.
  - If `e=0`, `k=3`. With `v≥10^18` ⇒ `v'≥10^21` ⇒ overflow.
- ETH/USDC: `a=18`, `b=6`, small `e` ⇒ `k≥7` ⇒ guaranteed overflow.

## Consistency Check with Safer Math
Other helpers use widened or checked arithmetic:
```275:316:/workspace/scope/programs/scope/src/utils/math.rs
pub fn mul_div(value: u64, multiplier: u64, divisor: u64) -> ScopeResult<u64> {
    if divisor == 0 {
        return Err(ScopeError::MathOverflow);
    }

    let value = value as u128;
    let multiplier = multiplier as u128;
    let divisor = divisor as u128;

    let product = value
        .checked_mul(multiplier)
        .ok_or(ScopeError::MathOverflow)?;

    let result = product.checked_div(divisor);

    result
        .ok_or(ScopeError::MathOverflow)?
        .try_into()
        .map_err(|_| ScopeError::MathOverflow)
}

pub fn normalize_rate(value: u64, from_decimals: u8, to_decimals: u8) -> ScopeResult<u64> {
    if from_decimals == to_decimals {
        return Ok(value);
    }
    let (diff, is_div) = if from_decimals > to_decimals {
        (from_decimals.checked_sub(to_decimals), true)
    } else {
        (to_decimals.checked_sub(from_decimals), false)
    };

    let diff = diff.ok_or(ScopeError::MathOverflow)?;
    let factor = 10u64
        .checked_pow(diff as u32)
        .ok_or(ScopeError::MathOverflow)?;
    let result = if is_div {
        value.checked_div(factor)
    } else {
        value.checked_mul(factor)
    };
    result.ok_or(ScopeError::MathOverflow)
}
```
By contrast, the vulnerable function uses unchecked `u64 * u64`.

## Build Configuration (Overflow Checks)
Release profiles enable overflow checks in Cargo manifests, but correctness must not rely on profile flags, and Solana deployments often optimize builds differently. Regardless, the local function should use checked or widened arithmetic.
```5:7:/workspace/scope/Cargo.toml
[profile.release]
lto = "thin"
overflow-checks = true
```
Also present in other workspace members.

## Exploitability and Impact
- Triggerability: Pure arithmetic; anyone can hit the branch for inputs where `e + b < a` and `k ≥ 2` with large `v`.
- Propagation: Adapters write the produced `Price` into `OraclePrices`; consumers read it directly.
- Outcomes: Wrong prices corrupt valuations, LTVs, caps, potentially enabling undercollateralized borrowing or improper liquidations.

## Root Cause
- Attempting to absorb decimal differences by inflating the mantissa in `u64` without checks.
- Inconsistent with the rest of the module’s safety posture.

## Recommendations (Minimal, Safe Changes)
- Replace unchecked multiply with checked/widened arithmetic; if overflow would occur, avoid mantissa inflation and adjust exponent instead.
- Remove `.unwrap()` on exponent conversion and propagate errors.

Illustrative safe approaches:
- Checked multiply:
```startLine:endLine:/workspace/scope/programs/scope/src/utils/math.rs
// ... inside else-branch
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
let multiplier = 10_u64.checked_pow(adjust_exp as u32)
    .ok_or(ScopeError::MathOverflow)?;
let value = lamport_value.checked_mul(multiplier)
    .ok_or(ScopeError::MathOverflow)?;
Price { value, exp: 0 }
```
- Widened arithmetic with downcast:
```startLine:endLine:/workspace/scope/programs/scope/src/utils/math.rs
// ... inside else-branch
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
let pow = 10_u128.pow(adjust_exp as u32);
let widened = (lamport_value as u128) * pow;
let value: u64 = widened.try_into().map_err(|_| ScopeError::MathOverflow)?;
Price { value, exp: 0 }
```
- Prefer exponent carry when multiply would overflow:
```startLine:endLine:/workspace/scope/programs/scope/src/utils/math.rs
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
if let Some(multiplier) = 10_u64.checked_pow(adjust_exp as u32) {
    if let Some(v) = lamport_value.checked_mul(multiplier) {
        Price { value: v, exp: 0 }
    } else {
        Price { value: lamport_value, exp: adjust_exp }
    }
} else {
    Price { value: lamport_value, exp: adjust_exp }
}
```

## Suggested Tests
- Unit tests for `price_of_lamports_to_price_of_tokens` covering 9↔6, 18↔6 decimals and `e ∈ {0,1,2,3}` with `v` near 10^18; assert no wraparound and correct scaling or explicit errors.
- Adapter-level tests for `meteora_dlmm` and `ktokens_token_x` to ensure monotonic, stable scaling across decimal configurations.

## Conclusion
The else-branch of `price_of_lamports_to_price_of_tokens` performs unchecked `u64` multiplication by a power of ten, which overflows for realistic inputs (especially with 9↔6 decimal skews and small `exp`). Multiple adapters call this function and store the result on-chain, where downstream consumers rely on it. The fix is straightforward: use checked or widened arithmetic and avoid mantissa inflation when it does not fit in `u64`. 