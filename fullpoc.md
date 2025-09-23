## Kamino Scope: Deep Security Analysis of Integer Overflow in `price_of_lamports_to_price_of_tokens`

### Executive Summary
- **Severity**: High (logic flaw) → DoS or incorrect price semantics depending on build/runtime
- **Component**: `scope/programs/scope/src/utils/math.rs::price_of_lamports_to_price_of_tokens`
- **Root cause**: Else-branch inflates a `u64` mantissa by `10^k` without checked/widened arithmetic and resets exponent to 0. For realistic inputs this multiply overflows a 64-bit integer.
- **Impact (this repo’s build config)**: With `overflow-checks = true` in release, overflow triggers a panic → transaction failure (DoS of price refresh). If compiled/run without release overflow checks, the same operation silently wraps → corrupted on-chain price. Either outcome is unsafe.
- **Propagation**: Called by live adapters (DLMM and KTokens Token‑X). Results are written to on-chain `OraclePrices` and consumed across Kamino protocols.

---

### Vulnerable Function and Context

Price representation:
```11:26:/workspace/scope/programs/scope/src/states.rs
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

Vulnerable function (offending multiply in else-branch):
```78:107:/workspace/scope/programs/scope/src/utils/math.rs
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

Upstream sources that can produce large mantissas with small exponents:
- From Q64.64 prices:
```45:76:/workspace/scope/programs/scope/src/utils/math.rs
pub fn q64x64_price_to_price(x64_price: U192) -> ScopeResult<Price> {
    const MAX_INTEGER_PART: u128 = u64::MAX as u128;

    let integer_part_u192 = x64_price >> U192::from(64);
    let integer_part_u128 = integer_part_u192.as_u128();

    let (exp, factor) = match integer_part_u128 {
        0 => (18, 10_u64.pow(18)),
        1..=9 => (17, 10_u64.pow(17)),
        10..=99 => (16, 10_u64.pow(16)),
        // ...
        100000000..=999999999 => (9, 10_u64.pow(9)),
        // ...
        100000000000000000..=MAX_INTEGER_PART => (0, 1),
        _ => return Err(ScopeError::OutOfRangeIntegralConversion),
    };
    let value_u192 = (x64_price * U192::from(factor)) >> U192::from(64);
    let value: u64 = value_u192.as_u64();
    Ok(Price { value, exp })
}
```

- From `u64_div_to_price`:
```109:141:/workspace/scope/programs/scope/src/utils/math.rs
pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // this implementation aims to keep as much precision as possible
    // choose exp to be the nearest power of 10 to the denominator
    // so that the result is in the range [0, 10^18]
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Creating a price by dividing by 0"),
        1..=10 => (0, 1_u64),
        11..=100 => (1, 10),
        // ...
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

Call sites that forward results into on-chain storage:
```82:86:/workspace/scope/programs/scope/src/oracles/meteora_dlmm.rs
let price = math::price_of_lamports_to_price_of_tokens(
    lamport_price,
    src_token_decimals.into(),
    dst_token_decimals.into(),
);
```

```153:158:/workspace/scope/programs/scope/src/oracles/ktokens_token_x.rs
price_of_lamports_to_price_of_tokens(
    price_lamport_to_lamport,
    share_decimals,
    token_decimals,
)
```

Written to `OraclePrices` in the refresh handler:
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

Release profile (this workspace):
```5:8:/workspace/scope/Cargo.toml
[profile.release]
lto = "thin"
overflow-checks = true
```

---

### Why and When It Overflows

- The else-branch computes `adjust_exp = token_a_decimals − (lamport_exp + token_b_decimals)` and then `mantissa' = lamport_value × 10^adjust_exp`, setting `exp' = 0`.
- Design bounds from `u64_div_to_price`: `lamport_value ≤ 10^18`.
- Let `U = u64::MAX ≈ 1.8446744 × 10^19`. If `adjust_exp ≥ 2`, the product `10^18 × 10^2 = 10^20 > U` → overflow.
- Therefore, any scenario with `token_a_decimals − token_b_decimals − lamport_exp ≥ 2` can overflow for large, yet legitimate mantissas.

Common Solana decimals make this plausible:
- Example: A=9 (SOL), B=6 (USDC). If upstream picks `lamport_exp ∈ {0,1}`, then `adjust_exp ∈ {2,3}` → overflow with mantissas near `10^18`.

Mathematically precise overflow predicate:
- Else-branch taken iff `lamport_exp + token_b_decimals < token_a_decimals`.
- Overflow guaranteed (worst case) iff `token_a_decimals − token_b_decimals − lamport_exp ≥ 2`.

---

### Actual Impact Given This Codebase

- This repository sets `overflow-checks = true` for release builds. Under that configuration, a `u64` overflow will panic in release, aborting the instruction. So the immediate impact is a deterministic transaction failure (DoS for price refresh) when inputs reach the unsafe region.
- If compiled/run in an environment where release overflow checks are not enabled, the same code silently wraps the mantissa modulo `2^64`, returns `exp = 0`, and can publish an invalid on-chain price. This is clearly unsafe but depends on build flags.

Either outcome is unacceptable for oracle code: panics deny updates; wraps corrupt data.

---

### Data-Flow: Which Adapters Can Trigger It?

- DLMM adapter path (Q64.64 → `q64x64_price_to_price` → conversion): the exponent selection often yields large `lamport_exp` (9..18) for typical integer parts, and with token decimals ≤ 9 this tends to take the if-branch (exponent-only adjust), avoiding the overflow-prone branch in normal ranges.
- KTokens Token‑X path (`u64_div_to_price` → conversion): when `num_shares` is small (early vaults or low supply), `lamport_exp` can be 0..1, and for common 9↔6 decimal pairs, `adjust_exp ≥ 2`. This readily reaches the overflow path.

Conclusion: KTokens Token‑X is the most realistic trigger path; DLMM can still be sensitive if unusual decimal combinations or integer-part buckets reduce the effective exponent.

---

### Proof-by-Construction Scenarios

- SOL/USDC-like pair: `token_a_decimals = 9`, `token_b_decimals = 6`.
  - Case 1: `lamport_exp = 1`, `lamport_value = 9.5 × 10^17` → `adjust_exp = 2` → product `9.5 × 10^19 > U`.
  - Case 2: `lamport_exp = 0`, `lamport_value = 1.0 × 10^18` → `adjust_exp = 3` → product `10^21`.

Under this repo’s release profile the instruction will panic on the multiply; if checks are disabled elsewhere the value wraps and returns `exp=0`.

---

### Inconsistency With Safer Math Elsewhere

Other helpers use widened/checked arithmetic; the vulnerable function is an outlier:
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

---

### Risk Assessment
- **Triggerability**: High for KTokens Token‑X with 9↔6 decimals and small `lamport_exp` values.
- **Effects**:
  - With overflow checks enabled (as here): instruction panics → price not updated (availability impact, potential griefing/DoS by crafting inputs feeding the unsafe region).
  - Without checks: wrong price published, potentially cascading into misvaluation in consumers.
- **Propagation**: `OraclePrices` persists the result and is consumed by KLend, KFarms, others.

---

### Recommendations (Safe, Minimal, Consistent)

Preferred: avoid mantissa inflation when unsafe; preserve exponent instead, or use wide arithmetic with checked downcast.

Option A — Keep mantissa, shift via exponent unless safe to inflate:

```text
// Pseudocode
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
if adjust_exp == 0 {
    return Price { value: lamport_value, exp: 0 };
}
let pow = 10_u64.checked_pow(u32::try_from(adjust_exp).map_err(..)?).ok_or(..)?;
if let Some(inflated) = lamport_value.checked_mul(pow) {
    Price { value: inflated, exp: 0 }
} else {
    // Overflow would occur; keep exponent instead of inflating mantissa
    Price { value: lamport_value, exp: adjust_exp }
}
```

Option B — Wide arithmetic and checked downcast:

```text
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
let pow = 10_u128.checked_pow(adjust_exp as u32).ok_or(..)?;
let widened = (lamport_value as u128).checked_mul(pow).ok_or(..)?;
let value: u64 = widened.try_into().map_err(|_| ScopeError::MathOverflow)?;
Price { value, exp: 0 }
```

Additionally:
- Replace `.unwrap()` on the `pow` exponent conversion with fallible conversions that return a `ScopeError`.
- Consider changing the signature to return `ScopeResult<Price>` to propagate arithmetic errors consistently with other helpers.

---

### Testing Guidance

Unit tests for `price_of_lamports_to_price_of_tokens`:
- 9↔6, 8↔6, 18↔6 decimal pairs.
- `lamport_exp ∈ {0,1,2,3}` and `lamport_value` near `10^18`.
- Assert: With the fix, no overflow occurs; either mantissa inflates safely or exponent absorbs the scaling.

End-to-end adapter tests:
- KTokens Token‑X across share supply regimes (small to large denominators), verifying monotonic, stable scaling without panics.
- DLMM across representative Q64.64 integer-part buckets to confirm else-branch behavior and safety.

---

### Notes on External Claims vs This Codebase
Some external reports assert “silent wrap in release builds.” In this repository, `overflow-checks = true` is explicitly enabled for release:
```5:8:/workspace/scope/Cargo.toml
[profile.release]
lto = "thin"
overflow-checks = true
```
Under that profile, an overflow will panic in release, not wrap. The underlying bug remains: the algorithm attempts unsafe mantissa inflation, causing either panics (DoS) or, if compiled/run without checks, silent corruption. The remedy is still to use checked/wide arithmetic or preserve scaling in the exponent.

---

### References
- Kamino Finance documentation: `https://docs.kamino.finance`
- Code references above show exact locations within this repository for the vulnerable function, helpers, call sites, and write path.

